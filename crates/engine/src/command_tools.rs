//! The person's own tools: a command (a program, a script, a `go run` line…) offered to the AI as a
//! tool of its own, with a name and what it's for. Listed in `config.toml`:
//!
//! ```toml
//! [[plugins.command]]
//! id = "word_count"                       # the tool becomes my_word_count
//! description = "Counts the words in a text."
//! command = "C:/tools/wc.exe"
//! args = ["--text", "{input}"]            # {input} is what the AI passes; without it, it goes on stdin
//! ```
//!
//! Every call asks the person first (it runs a program of theirs), and a tool runs in Clean room
//! only when its `clean_room` says it never looks inside the original.

use crate::registry::{RuntimeTool, custom_toolset};
use crate::session::Session;
use crate::tool::{Ctx, Effects, Requires, ToolOutput};
use craft_anything_evidence::{Authority, Confidence, Environment, EvidenceBuilder, Subject, ToolError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// What every tool id starts with, so a tool of the person's never takes a built-in tool's id.
pub const PREFIX: &str = "my_";
/// The toolset the person's tools are in.
pub const TOOLSET: &str = "my_tools";
/// Where an argument takes the AI's input.
pub const INPUT: &str = "{input}";
/// How long one call may take unless the tool says otherwise.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// The most output kept from each stream.
const OUTPUT_CAP: usize = 256 * 1024;
/// Most tools of the person's.
const MAX_TOOLS: usize = 200;

/// One tool of the person's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandTool {
    /// The tool's id after [`PREFIX`]: lower-case letters, digits and `_`.
    pub id: String,
    /// The name people see (default: the id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What it does and when to use it, for the AI.
    #[serde(default)]
    pub description: String,
    /// The program to run.
    pub command: String,
    /// Its arguments; [`INPUT`] in one is replaced by what the AI passes.
    #[serde(default)]
    pub args: Vec<String>,
    /// The folder it runs in (default: the app's own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    /// Seconds one call may take (default: ten minutes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// It never looks inside the original, so it may run in Clean room. Off unless set.
    #[serde(default)]
    pub clean_room: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

impl CommandTool {
    /// The tool's id as the AI sees it.
    pub fn tool_id(&self) -> String {
        format!("{PREFIX}{}", self.id)
    }

    pub fn display_name(&self) -> &str {
        self.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&self.id)
    }

    /// What's wrong with it, if anything.
    pub fn problem(&self) -> Option<String> {
        if !RuntimeTool::valid_id(&self.tool_id()) || self.id.is_empty() {
            return Some(format!("Tool id {:?} must be lower-case letters, digits and _ (at most 61).", self.id));
        }
        if self.command.trim().is_empty() {
            return Some(format!("The {} tool needs a command to run.", self.id));
        }
        if self.description.trim().is_empty() {
            return Some(format!("The {} tool needs a description, so the AI knows when to use it.", self.id));
        }
        None
    }

    /// The command line, for people.
    pub fn place(&self) -> String {
        std::iter::once(self.command.as_str()).chain(self.args.iter().map(String::as_str)).collect::<Vec<_>>().join(" ")
    }

    /// Whether the AI's input goes in an argument (else on standard input).
    fn input_in_args(&self) -> bool {
        self.args.iter().any(|a| a.contains(INPUT))
    }

    /// The arguments with the input in place of [`INPUT`].
    fn args_with(&self, input: &str) -> Vec<String> {
        self.args.iter().map(|a| a.replace(INPUT, input)).collect()
    }

    fn schema(&self) -> Value {
        let goes = if self.input_in_args() { "It goes in the command's arguments." } else { "It's written to the command's standard input." };
        json!({
            "type": "object",
            "properties": {"input": {"type": "string", "description": format!("What to give the tool. {goes}")}},
            "additionalProperties": false,
        })
    }
}

/// The session's tools of the person's, by tool id.
#[derive(Default)]
struct CommandTools(HashMap<&'static str, CommandTool>);

/// Read the person's tools into `session` and add them to `tools`. A tool with a problem, or whose
/// id is taken, is skipped and logged.
pub fn install(session: &mut Session, tools: &mut Vec<RuntimeTool>) {
    let mut mine = CommandTools::default();
    let configs: Vec<CommandTool> = session.settings.plugins.command.iter().filter(|t| t.enabled).take(MAX_TOOLS).cloned().collect();
    let Some(set) = (!configs.is_empty()).then(|| custom_toolset(TOOLSET)).flatten() else {
        session.ext.insert(Arc::new(mine));
        return;
    };
    for t in configs {
        if let Some(problem) = t.problem() {
            log::error!("your tool skipped: {problem}");
            continue;
        }
        let id = t.tool_id();
        if tools.iter().any(|x| x.id == id) || mine.0.contains_key(id.as_str()) {
            log::error!("your tool {} skipped: its id {id:?} is taken", t.id);
            continue;
        }
        let leaked: &'static str = Box::leak(id.clone().into_boxed_str());
        if t.clean_room {
            crate::clean_room::allow_also(leaked);
        }
        tools.push(RuntimeTool {
            id,
            title: t.display_name().to_string(),
            summary: format!("{} (a tool of the person's own: it runs `{}`, and asks them first)", t.description.trim(), t.place()),
            beginner: format!("Your own tool \"{}\".", t.display_name()),
            toolset: set,
            // It runs a program of the person's, which could do anything: it always asks first.
            effects: Effects::LAUNCHES_PROCESS | Effects::EXECUTES_FOREIGN_CODE | Effects::WRITES_FILESYSTEM | Effects::ACCESSES_NETWORK,
            requires: Requires::Nothing,
            schema: t.schema(),
            run: call_tool,
        });
        mine.0.insert(leaked, t);
    }
    session.ext.insert(Arc::new(mine));
}

/// The function behind every tool of the person's: run its command with the AI's input and
/// record what it printed and how it ended.
fn call_tool(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let mine = ctx.session.ext.get::<CommandTools>().ok_or_else(|| ToolError::internal("your tools aren't loaded in this session"))?;
    let tool = mine.0.get(ctx.tool).cloned().ok_or_else(|| ToolError::internal(format!("no tool of yours is {}", ctx.tool)))?;
    let input = params.get("input").and_then(Value::as_str).unwrap_or_default().to_string();
    ctx.report(None, format!("Running your tool {}…", tool.display_name()));
    let cancel = ctx.cancel.clone();
    let mut p = craft_anything_platform::Process::new(tool.command.trim())
        .args(tool.args_with(&input))
        .timeout(tool.timeout_secs.filter(|s| *s > 0).map_or(DEFAULT_TIMEOUT, Duration::from_secs))
        .stdout_cap(OUTPUT_CAP)
        .stderr_cap(OUTPUT_CAP)
        .cancel_when(move || cancel.is_cancelled())
        .label(tool.display_name().to_string())
        .remediation(format!("Check the command of your tool {} in Settings → Plugins.", tool.display_name()));
    if !tool.input_in_args() {
        p = p.stdin_bytes(input.into_bytes());
    }
    if let Some(dir) = tool.cwd.as_ref().filter(|d| !d.as_os_str().is_empty()) {
        p = p.cwd(dir.clone());
    }
    let out = p.run()?;
    let (stdout, stderr) = (out.stdout_text(), out.stderr_text());
    let summary = match out.code {
        Some(0) => format!("{} finished.", tool.display_name()),
        Some(code) => format!("{} ended with exit code {code}.", tool.display_name()),
        None => format!("{} was stopped.", tool.display_name()),
    };
    let result = json!({
        "exit_code": out.code,
        "success": out.success,
        "stdout": stdout,
        "stderr": stderr,
        "stdout_truncated": out.stdout_truncated,
        "stderr_truncated": out.stderr_truncated,
    });
    let env = Environment {
        clean_room: ctx.session.clean_room(),
        engine: format!("my_tool:{}", tool.id),
        engine_version: None,
        host: craft_anything_evidence::host_triple(),
        language_id: None,
    };
    let e = EvidenceBuilder::new(ctx.tool, Subject { kind: "my_tool".into(), sha256: None, name: Some(tool.display_name().to_string()) }, env)
        .params(params)
        .authority(Authority::OutsideService)
        .confidence(Confidence::Observed)
        .result(result)
        .summary(summary)
        .build()?;
    Ok(ToolOutput::evidence(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(id: &str, command: &str, args: &[&str]) -> CommandTool {
        CommandTool {
            id: id.into(),
            name: None,
            description: "Says something back.".into(),
            command: command.into(),
            args: args.iter().map(|a| a.to_string()).collect(),
            cwd: None,
            timeout_secs: None,
            clean_room: false,
            enabled: true,
        }
    }

    #[test]
    fn a_tool_needs_an_id_a_command_and_a_description() {
        assert!(tool("echo", "echo", &[]).problem().is_none());
        assert!(tool("Bad Id", "echo", &[]).problem().unwrap().contains("lower-case"));
        assert!(tool("", "echo", &[]).problem().is_some());
        assert!(tool("echo", " ", &[]).problem().unwrap().contains("command"));
        assert!(CommandTool { description: " ".into(), ..tool("echo", "echo", &[]) }.problem().unwrap().contains("description"));
        assert_eq!(tool("echo", "echo", &[]).tool_id(), "my_echo");
    }

    #[test]
    fn the_input_goes_in_its_argument_or_on_stdin() {
        let t = tool("x", "prog", &["--text", "{input}", "--n={input}"]);
        assert!(t.input_in_args());
        assert_eq!(t.args_with("hi"), ["--text", "hi", "--n=hi"]);
        assert!(!tool("x", "prog", &["--fast"]).input_in_args());
        assert!(t.schema()["properties"]["input"]["description"].as_str().unwrap().contains("arguments"));
    }

    #[test]
    fn saved_tools_read_back_on_and_kept_out_of_clean_room() {
        let parsed: crate::plugins::PluginSettings =
            toml::from_str("[[command]]\nid = \"wc\"\ndescription = \"Counts words.\"\ncommand = \"wc\"\nargs = [\"{input}\"]\n").unwrap();
        let t = &parsed.command[0];
        assert!(t.enabled && !t.clean_room, "on, and Deep-only, unless set");
        assert_eq!(t.place(), "wc {input}");
    }

    #[test]
    fn enabled_tools_become_tools_that_ask_first() {
        let mut s = crate::testing::session();
        s.settings.plugins.command = vec![
            tool("say_hi_test_only", "echo", &["{input}"]),
            CommandTool { enabled: false, ..tool("off_test_only", "echo", &[]) },
            tool("Bad", "echo", &[]),
            CommandTool { clean_room: true, ..tool("safe_test_only", "echo", &[]) },
        ];
        let mut tools = Vec::new();
        install(&mut s, &mut tools);
        let ids: Vec<&str> = tools.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["my_say_hi_test_only", "my_safe_test_only"]);
        assert!(tools.iter().all(|t| t.effects.needs_consent()), "a program of the person's always asks first");
        assert!(crate::clean_room::allows("my_safe_test_only") && !crate::clean_room::allows("my_say_hi_test_only"));
    }

    /// A real call: the input reaches the program, and what it prints comes back.
    #[test]
    fn a_call_runs_the_command_and_returns_its_output() {
        let (command, args): (&str, Vec<&str>) = if cfg!(windows) { ("cmd", vec!["/C", "echo", "{input}"]) } else { ("sh", vec!["-c", "echo \"$0\"", "{input}"]) };
        let mut s = crate::testing::session();
        s.settings.plugins.command = vec![tool("echo_call_test_only", command, &args)];
        let mut tools = Vec::new();
        install(&mut s, &mut tools);
        let mut ctx =
            Ctx { tool: "my_echo_call_test_only", session: &mut s, cancel: crate::CancelToken::default(), progress: &|_| {}, caller: crate::Caller::Cli };
        let out = call_tool(&mut ctx, json!({"input": "hola"})).unwrap().to_json();
        let text = out.to_string();
        assert!(text.contains("hola") && text.contains("\"exit_code\":0"), "{text}");
    }
}
