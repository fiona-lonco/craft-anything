//! Pictures and videos from a model of the person's own ([`CustomModel`]): any web API (OpenAI,
//! Google, xAI, Replicate, a local Stable Diffusion…) through a URL, a JSON body with placeholders
//! and headers, or a program on this computer that writes the file. [`PRESETS`] fills those in for
//! the best-known services, so the person only adds their key.

use super::{AssetProvider, AssetRequest, CustomKind, CustomModel, Generated, Shape};
use craft_anything_evidence::{ErrorCode, ToolError};
use serde_json::{Value, json};
use std::time::Duration;

/// The provider id of a web API of the person's own.
pub const HTTP: &str = "http";
/// The provider id of a program of the person's own.
pub const COMMAND: &str = "command";
/// Where the person's key goes in a URL or a header.
pub const KEY: &str = "{key}";

/// What a picture may take unless the model says otherwise.
const IMAGE_WAIT: Duration = Duration::from_secs(10 * 60);
/// What a video may take unless the model says otherwise.
const VIDEO_WAIT: Duration = Duration::from_secs(30 * 60);

/// The values the placeholders take for one request.
#[derive(Debug, Clone)]
pub struct Fill {
    pub prompt: String,
    pub width: u32,
    pub height: u32,
    /// `1024x1024`, `1536x1024` or `1024x1536`: the sizes most picture APIs take.
    pub size: &'static str,
    pub aspect_ratio: &'static str,
    pub image_url: String,
    pub kind: &'static str,
    pub output: String,
    pub key: String,
}

impl Fill {
    fn new(prompt: &str, shape: Shape, image_url: Option<&str>, kind: &'static str, key: &str) -> Fill {
        let (width, height, size, aspect_ratio) = match shape {
            Shape::Square => (1024, 1024, "1024x1024", "1:1"),
            Shape::Wide => (1024, 576, "1536x1024", "16:9"),
            Shape::Tall => (576, 1024, "1024x1536", "9:16"),
        };
        Fill {
            prompt: prompt.to_string(),
            width,
            height,
            size,
            aspect_ratio,
            image_url: image_url.unwrap_or_default().to_string(),
            kind,
            output: String::new(),
            key: key.to_string(),
        }
    }

    /// Values to check a body template with.
    pub fn sample() -> Fill {
        Fill::new("a \"quoted\" test", Shape::Square, None, "image", "k")
    }

    fn pairs(&self) -> [(&'static str, String); 10] {
        [
            ("{prompt}", self.prompt.clone()),
            ("{width}", self.width.to_string()),
            ("{height}", self.height.to_string()),
            ("{size}", self.size.to_string()),
            ("{aspect_ratio}", self.aspect_ratio.to_string()),
            ("{image_url}", self.image_url.clone()),
            ("{kind}", self.kind.to_string()),
            ("{output}", self.output.clone()),
            (KEY, self.key.clone()),
            ("{seconds}", "8".to_string()),
        ]
    }
}

/// `text` with the placeholders filled in as they are (a URL, a header, a program's argument).
pub fn fill_text(text: &str, fill: &Fill) -> String {
    fill.pairs().iter().fold(text.to_string(), |t, (k, v)| t.replace(k, v))
}

/// A JSON body template filled in: text values are escaped for JSON (so a prompt with quotes
/// stays valid), and the result must parse.
pub fn fill_json(template: &str, fill: &Fill) -> Result<Value, ToolError> {
    let escaped = fill.pairs().map(|(k, v)| {
        let quoted = serde_json::to_string(&v).unwrap_or_default();
        (k, quoted.get(1..quoted.len().saturating_sub(1)).unwrap_or_default().to_string())
    });
    let text = escaped.iter().fold(template.to_string(), |t, (k, v)| t.replace(k, v));
    serde_json::from_str(&text).map_err(|e| {
        ToolError::new(ErrorCode::BadSettings, format!("The body of your model isn't valid JSON once it's filled in: {e}"))
            .with_remediation("Fix it in Settings → Picture and video AI: put {prompt} inside quotes, and numbers like {width} without them.")
    })
}

/// A ready setup for a well-known service: the person adds their key and, where it asks, the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    /// Pictures or videos.
    pub video: bool,
    pub kind: CustomKind,
    pub endpoint: &'static str,
    pub body: &'static str,
    pub headers: &'static [(&'static str, &'static str)],
    pub result: &'static str,
    /// What one result costs, as listed when it was added (the person can change it).
    pub usd_each: f64,
    /// Where the key comes from.
    pub keys_page: &'static str,
}

impl Preset {
    pub fn model(&self) -> CustomModel {
        CustomModel {
            kind: self.kind,
            endpoint: self.endpoint.into(),
            usd_each: self.usd_each,
            body: self.body.into(),
            headers: self.headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            result: self.result.into(),
            args: Vec::new(),
            timeout_secs: None,
            key: String::new(),
        }
    }
}

/// Direct APIs of the best-known services, and a local Stable Diffusion. Videos from those
/// services' own APIs come back later from a job to poll, so they go through fal.ai (Veo, Kling…),
/// an API that answers with the video, or a program.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "openai",
        name: "OpenAI (gpt-image-1)",
        video: false,
        kind: CustomKind::Http,
        endpoint: "https://api.openai.com/v1/images/generations",
        body: r#"{"model": "gpt-image-1", "prompt": "{prompt}", "size": "{size}", "n": 1}"#,
        headers: &[("Authorization", "Bearer {key}")],
        result: "/data/0/b64_json",
        usd_each: 0.04,
        keys_page: "https://platform.openai.com/api-keys",
    },
    Preset {
        id: "gemini",
        name: "Google Gemini (Nano Banana)",
        video: false,
        kind: CustomKind::Http,
        endpoint: "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
        body: r#"{"contents": [{"parts": [{"text": "{prompt}"}]}], "generationConfig": {"responseModalities": ["IMAGE"], "imageConfig": {"aspectRatio": "{aspect_ratio}"}}}"#,
        headers: &[("x-goog-api-key", "{key}")],
        result: "",
        usd_each: 0.04,
        keys_page: "https://aistudio.google.com/apikey",
    },
    Preset {
        id: "imagen",
        name: "Google Imagen 4",
        video: false,
        kind: CustomKind::Http,
        endpoint: "https://generativelanguage.googleapis.com/v1beta/models/imagen-4.0-generate-001:predict",
        body: r#"{"instances": [{"prompt": "{prompt}"}], "parameters": {"sampleCount": 1, "aspectRatio": "{aspect_ratio}"}}"#,
        headers: &[("x-goog-api-key", "{key}")],
        result: "/predictions/0/bytesBase64Encoded",
        usd_each: 0.04,
        keys_page: "https://aistudio.google.com/apikey",
    },
    Preset {
        id: "xai",
        name: "xAI (Grok image)",
        video: false,
        kind: CustomKind::Http,
        endpoint: "https://api.x.ai/v1/images/generations",
        body: r#"{"model": "grok-2-image", "prompt": "{prompt}", "n": 1, "response_format": "b64_json"}"#,
        headers: &[("Authorization", "Bearer {key}")],
        result: "/data/0/b64_json",
        usd_each: 0.07,
        keys_page: "https://console.x.ai",
    },
    Preset {
        id: "replicate",
        name: "Replicate (FLUX 1.1 Pro)",
        video: false,
        kind: CustomKind::Http,
        endpoint: "https://api.replicate.com/v1/models/black-forest-labs/flux-1.1-pro/predictions",
        body: r#"{"input": {"prompt": "{prompt}", "aspect_ratio": "{aspect_ratio}", "output_format": "png"}}"#,
        headers: &[("Authorization", "Bearer {key}"), ("Prefer", "wait")],
        result: "",
        usd_each: 0.04,
        keys_page: "https://replicate.com/account/api-tokens",
    },
    Preset {
        id: "a1111",
        name: "Stable Diffusion on this computer (AUTOMATIC1111 / Forge)",
        video: false,
        kind: CustomKind::Http,
        endpoint: "http://127.0.0.1:7860/sdapi/v1/txt2img",
        body: r#"{"prompt": "{prompt}", "width": {width}, "height": {height}, "steps": 25}"#,
        headers: &[],
        result: "/images/0",
        usd_each: 0.0,
        keys_page: "",
    },
    Preset {
        id: "fal_video",
        name: "fal.ai (Veo 3 Fast)",
        video: true,
        kind: CustomKind::Fal,
        endpoint: "fal-ai/veo3/fast",
        body: "",
        headers: &[],
        result: "",
        usd_each: 3.2,
        keys_page: "https://fal.ai/dashboard/keys",
    },
];

/// Where a picture or video sits in an answer when the model doesn't say: the usual keys of the
/// common APIs, looked for anywhere in it (Gemini puts its picture in a part after its text).
fn find_result(v: &Value) -> Option<String> {
    for p in ["/data/0/b64_json", "/data/0/url", "/images/0/url", "/image/url", "/video/url", "/videos/0/url", "/output/0", "/output", "/images/0", "/url"] {
        if let Some(s) = v.pointer(p).and_then(Value::as_str).filter(|s| !s.is_empty()) {
            return Some(s.to_string());
        }
    }
    deep_find(v, 0)
}

fn deep_find(v: &Value, depth: usize) -> Option<String> {
    if depth > 12 {
        return None;
    }
    match v {
        Value::Object(map) => {
            for k in ["b64_json", "bytesBase64Encoded", "base64"] {
                if let Some(s) = map.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()) {
                    return Some(s.to_string());
                }
            }
            for k in ["inlineData", "inline_data"] {
                if let Some(s) = map.get(k).and_then(|d| d.get("data")).and_then(Value::as_str).filter(|s| !s.is_empty()) {
                    return Some(s.to_string());
                }
            }
            map.values().find_map(|x| deep_find(x, depth + 1))
        }
        Value::Array(items) => items.iter().find_map(|x| deep_find(x, depth + 1)),
        _ => None,
    }
}

/// The file a result names: a web address is downloaded, a `data:` URI or bare base64 decoded.
fn result_bytes(found: &str) -> Result<(Vec<u8>, String), ToolError> {
    let found = found.trim();
    if found.starts_with("https://") || found.starts_with("http://") || found.starts_with("data:") {
        return Ok((super::fal::download(found)?, found.chars().take(500).collect()));
    }
    Ok((super::fal::base64_decode(found)?, "(sent in the answer)".into()))
}

/// The person's model, its fill and how long it may take, for a picture or a video.
fn setup<'a>(req: &'a AssetRequest, key: &str) -> Result<(&'a CustomModel, Fill, Duration), ToolError> {
    let (custom, fill, wait) = match *req {
        AssetRequest::Image { model, prompt, shape } => (model.custom.as_ref(), Fill::new(prompt, shape, None, "image", key), IMAGE_WAIT),
        AssetRequest::Video { model, prompt, shape, image_url } => (model.custom.as_ref(), Fill::new(prompt, shape, image_url, "video", key), VIDEO_WAIT),
        AssetRequest::Model3d { .. } => return Err(ToolError::new(ErrorCode::FeatureMissing, "Your own models make pictures and videos, not 3D models.")),
    };
    let custom = custom.ok_or_else(|| ToolError::internal("this model has no setup of the person's own"))?;
    if let Some(problem) = custom.problem() {
        return Err(ToolError::new(ErrorCode::BadSettings, problem).with_remediation("Fix your model in Settings → Picture and video AI."));
    }
    Ok((custom, fill, custom.timeout_secs.filter(|s| *s > 0).map_or(wait, Duration::from_secs)))
}

/// Any web API of the person's own, as an [`AssetProvider`].
pub struct Http;

impl AssetProvider for Http {
    fn id(&self) -> &'static str {
        HTTP
    }

    fn name(&self) -> &'static str {
        "Your own web API"
    }

    fn needs_key(&self) -> bool {
        false
    }

    fn generate(&self, _key: &str, req: &AssetRequest) -> Result<Generated, ToolError> {
        let (m, fill, wait) = setup(req, "")?;
        let fill = Fill { key: m.key.clone(), ..fill };
        let url = fill_text(&m.endpoint(), &fill);
        let body = if m.body.trim().is_empty() {
            json!({"prompt": fill.prompt, "width": fill.width, "height": fill.height, "aspect_ratio": fill.aspect_ratio})
        } else {
            fill_json(&m.body, &fill)?
        };
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .user_agent("CraftAnything")
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(wait))
            .timeout_recv_body(Some(Duration::from_secs(10 * 60)))
            .build()
            .new_agent();
        let mut call = agent.post(&url).header("Content-Type", "application/json");
        for (k, v) in &m.headers {
            call = call.header(k.as_str(), fill_text(v, &fill).as_str());
        }
        let resp = call.send(body.to_string()).map_err(|e| {
            ToolError::new(ErrorCode::EngineMissing, format!("Couldn't reach your model at {}: {e}", m.endpoint()))
                .with_remediation("Check its address in Settings → Picture and video AI, and that it's running if it's on this computer.")
        })?;
        let status = resp.status().as_u16();
        let media = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or_default().to_ascii_lowercase();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut std::io::Read::take(resp.into_body().into_reader(), super::fal::MAX_BYTES + 1), &mut bytes)
            .map_err(|e| ToolError::new(ErrorCode::EngineMissing, format!("The answer broke off: {e}")))?;
        if !(200..300).contains(&status) {
            let text: String = String::from_utf8_lossy(&bytes).chars().take(400).collect();
            let (code, fix) = match status {
                401 | 403 => (ErrorCode::BadSettings, "The key was refused: check it in Settings → Picture and video AI."),
                402 | 429 => (ErrorCode::OverLimit, "The account is out of credit or over its limit."),
                _ => (ErrorCode::EngineMissing, "Check the model's address and body in Settings → Picture and video AI."),
            };
            return Err(ToolError::new(code, format!("Your model answered {status}: {text}")).with_remediation(fix));
        }
        // Some APIs answer with the file itself.
        if media.starts_with("image/") || media.starts_with("video/") {
            return Ok(Generated { bytes, url: url.clone(), cost_usd: req.listed_cost() });
        }
        let v: Value = serde_json::from_slice(&bytes)
            .map_err(|e| ToolError::new(ErrorCode::GarbledOutput, format!("Your model sent an answer that isn't JSON or a file: {e}")))?;
        let found = if m.result.trim().is_empty() { find_result(&v) } else { v.pointer(m.result.trim()).and_then(Value::as_str).map(str::to_string) };
        let found = found.filter(|s| !s.trim().is_empty()).ok_or_else(|| {
            ToolError::new(ErrorCode::GarbledOutput, "Your model's answer didn't have the picture or video where it was looked for.")
                .with_remediation("Set where the answer has it (like /data/0/b64_json) in Settings → Picture and video AI.")
                .with_details(json!({"answer": v.to_string().chars().take(500).collect::<String>()}))
        })?;
        let (bytes, at) = result_bytes(&found)?;
        Ok(Generated { bytes, url: at, cost_usd: req.listed_cost() })
    }
}

/// A program of the person's own that writes the picture or video, as an [`AssetProvider`].
pub struct Command;

impl AssetProvider for Command {
    fn id(&self) -> &'static str {
        COMMAND
    }

    fn name(&self) -> &'static str {
        "A program on this computer"
    }

    fn needs_key(&self) -> bool {
        false
    }

    fn generate(&self, _key: &str, req: &AssetRequest) -> Result<Generated, ToolError> {
        let (m, fill, wait) = setup(req, "")?;
        let ext = if matches!(req, AssetRequest::Video { .. }) { "mp4" } else { "png" };
        let dir = std::env::temp_dir().join(format!("craft-anything-asset-{}-{}", std::process::id(), unique()));
        std::fs::create_dir_all(&dir).map_err(|e| ToolError::io(dir.display(), e))?;
        let out = dir.join(format!("result.{ext}"));
        let fill = Fill { output: out.display().to_string(), key: m.key.clone(), ..fill };
        let args: Vec<String> = m.args.iter().map(|a| fill_text(a, &fill)).collect();
        let run = craft_anything_platform::Process::new(m.endpoint())
            .args(&args)
            .timeout(wait)
            .label("your picture or video program")
            .remediation("Check the program and its arguments in Settings → Picture and video AI.")
            .run_checked();
        let read = run.and_then(|_| {
            let meta = std::fs::metadata(&out).map_err(|_| {
                ToolError::new(ErrorCode::ProgramFailed, "Your program finished but wrote no file at {output}.")
                    .with_remediation("Make it save the picture or video to the path it gets in place of {output}.")
            })?;
            if meta.len() > super::fal::MAX_BYTES {
                return Err(ToolError::new(ErrorCode::OverLimit, "The file your program made is too large."));
            }
            std::fs::read(&out).map_err(|e| ToolError::io(out.display(), e))
        });
        let _ = std::fs::remove_dir_all(&dir);
        Ok(Generated { bytes: read?, url: format!("made by {}", m.endpoint()), cost_usd: req.listed_cost() })
    }
}

/// A number no other call in this process gets.
fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    fn image(m: &CustomModel, prompt: &str) -> Result<Generated, ToolError> {
        let model = m.as_image();
        let p = if m.kind == CustomKind::Http { Http.generate("", &AssetRequest::Image { model: &model, prompt, shape: Shape::Wide }) } else { Command.generate("", &AssetRequest::Image { model: &model, prompt, shape: Shape::Wide }) };
        p
    }

    /// A stand-in API answering one request with `body` as `media`; returns its address and what
    /// it was sent.
    fn serve(media: &'static str, body: Vec<u8>) -> (String, std::sync::mpsc::Receiver<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = l.accept().unwrap();
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let (mut head, mut len) = (String::new(), 0usize);
            loop {
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
                head.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            let mut sent = vec![0; len];
            r.read_exact(&mut sent).unwrap();
            tx.send(format!("{head}{}", String::from_utf8_lossy(&sent))).unwrap();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: {media}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(&body).unwrap();
        });
        (base, rx)
    }

    #[test]
    fn a_prompt_with_quotes_keeps_the_body_valid() {
        let v = fill_json(r#"{"prompt": "{prompt}", "width": {width}, "size": "{size}"}"#, &Fill::new("say \"hi\"\nnow", Shape::Wide, None, "image", "k")).unwrap();
        assert_eq!(v, json!({"prompt": "say \"hi\"\nnow", "width": 1024, "size": "1536x1024"}));
        assert!(fill_json("{not json", &Fill::sample()).is_err());
        assert_eq!(fill_text("Bearer {key}", &Fill::new("x", Shape::Square, None, "image", "sk-1")), "Bearer sk-1");
    }

    /// OpenAI's shape: the key in a header, the picture as base64 at /data/0/b64_json.
    #[test]
    fn a_web_api_with_a_preset() {
        let (base, sent) = serve("application/json", json!({"data": [{"b64_json": "UE5HIQ=="}]}).to_string().into_bytes());
        let preset = PRESETS.iter().find(|p| p.id == "openai").unwrap();
        let m = CustomModel { endpoint: format!("{base}/v1/images/generations"), key: "sk-test".into(), ..preset.model() };
        let g = image(&m, "a \"red\" door").unwrap();
        assert_eq!((g.bytes.as_slice(), g.cost_usd), (&b"PNG!"[..], 0.04));
        let sent = sent.recv().unwrap();
        assert!(sent.starts_with("POST /v1/images/generations "), "{sent}");
        assert!(sent.to_ascii_lowercase().contains("authorization: bearer sk-test"), "{sent}");
        assert!(sent.contains(r#""prompt":"a \"red\" door""#) && sent.contains(r#""size":"1536x1024""#), "{sent}");
    }

    /// Gemini puts the picture after a text part; nothing says where, and it's found.
    #[test]
    fn a_result_is_found_where_the_usual_apis_put_it() {
        let gemini = json!({"candidates": [{"content": {"parts": [{"text": "here"}, {"inlineData": {"mimeType": "image/png", "data": "QUJD"}}]}}]});
        assert_eq!(find_result(&gemini).as_deref(), Some("QUJD"));
        assert_eq!(find_result(&json!({"predictions": [{"bytesBase64Encoded": "WFla"}]})).as_deref(), Some("WFla"));
        assert_eq!(find_result(&json!({"output": "https://x/out.png"})).as_deref(), Some("https://x/out.png"));
        assert_eq!(find_result(&json!({"images": ["QkFTRTY0"]})).as_deref(), Some("QkFTRTY0"), "AUTOMATIC1111");
        assert_eq!(find_result(&json!({"status": "ok"})), None);
    }

    /// An API that answers with the file itself.
    #[test]
    fn a_web_api_that_answers_with_the_file() {
        let (base, _sent) = serve("image/png", b"RAW PNG".to_vec());
        let m = CustomModel { kind: CustomKind::Http, endpoint: format!("{base}/make"), ..Default::default() };
        assert_eq!(image(&m, "x").unwrap().bytes, b"RAW PNG");
    }

    /// A program gets the prompt and writes the file at {output}.
    #[test]
    fn a_program_writes_the_file() {
        let (program, args): (&str, Vec<String>) = if cfg!(windows) {
            ("cmd", vec!["/C".into(), "echo".into(), "{prompt}>{output}".into()])
        } else {
            ("sh", vec!["-c".into(), "printf '%s' \"$0\" > \"$1\"".into(), "{prompt}".into(), "{output}".into()])
        };
        let m = CustomModel { kind: CustomKind::Command, endpoint: program.into(), args, ..Default::default() };
        let g = image(&m, "hola").unwrap();
        assert!(String::from_utf8_lossy(&g.bytes).contains("hola"), "{:?}", g.bytes);
        // One that finishes without writing the file says so.
        if cfg!(windows) {
            let lazy = CustomModel { args: vec!["/C".into(), "echo".into(), "nothing".into(), "{output}".into()], ..m.clone() };
            assert!(image(&lazy, "x").unwrap_err().message.contains("wrote no file"));
        }
    }

    #[test]
    fn each_kind_says_what_it_needs() {
        let m = |kind, endpoint: &str| CustomModel { kind, endpoint: endpoint.into(), ..Default::default() };
        assert!(m(CustomKind::Http, "ftp://x").problem().is_some());
        assert!(m(CustomKind::Http, "http://127.0.0.1:7860/sdapi/v1/txt2img").problem().is_none());
        assert!(CustomModel { body: "{oops".into(), ..m(CustomKind::Http, "https://x") }.problem().is_some());
        assert!(CustomModel { result: "data/0".into(), ..m(CustomKind::Http, "https://x") }.problem().is_some());
        assert!(m(CustomKind::Command, "sd.exe").problem().unwrap().contains("{output}"));
        assert!(CustomModel { args: vec!["-o".into(), "{output}".into()], ..m(CustomKind::Command, "sd.exe") }.problem().is_none());
        for p in PRESETS {
            let keyed = CustomModel { key: "k".into(), ..p.model() };
            assert!(keyed.problem().is_none(), "{} is ready once a key is added", p.id);
            let asks = p.headers.iter().any(|(_, v)| v.contains(KEY));
            assert_eq!(p.model().problem().is_some(), asks, "{}: a key is asked for only where it's used", p.id);
            assert_eq!(p.model().provider(), if p.kind == CustomKind::Fal { super::super::fal::ID } else { HTTP });
        }
    }
}
