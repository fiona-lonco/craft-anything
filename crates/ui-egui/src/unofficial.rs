//! What this unofficial version asks on its first start: whether to show it in ArtCraft Launcher
//! as an installed app, and whether to be told about its new versions (from its own releases).
//!
//! The launcher lists what it installed in `%LOCALAPPDATA%\ArtCraft Launcher\installed.json`; this
//! version adds itself there, and pins its version in the launcher's settings so the launcher never
//! replaces it with the official release. The launcher keeps its settings in memory while it runs,
//! so it has to be closed for the pin to stay.

use crate::CraftApp;
use crate::i18n::tr;
use crate::theme::{self, Tokens};
use crate::widgets;
use egui::RichText;
use std::path::{Path, PathBuf};

/// The launcher's id for this app.
const APP_ID: &str = "craft-anything";

/// The official release this version is based on (`0.6.1` for `0.6.1-fl.1`): the launcher pins it.
fn base_version() -> &'static str {
    let v = env!("CARGO_PKG_VERSION");
    v.split_once('-').map_or(v, |(base, _)| base)
}

/// The launcher's own folders, when it's installed for this account: (its record of installs, its
/// settings file).
fn launcher_files() -> Option<(PathBuf, PathBuf)> {
    if !cfg!(windows) {
        return None;
    }
    let local = PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
    let roaming = PathBuf::from(std::env::var_os("APPDATA")?);
    let data = local.join("ArtCraft Launcher");
    data.is_dir().then(|| (data.join("installed.json"), roaming.join("ai.storyteller.artcraft-launcher").join("data").join("app.ron")))
}

/// Whether ArtCraft Launcher is on this computer.
pub fn launcher_found() -> bool {
    launcher_files().is_some()
}

/// Whether ArtCraft Launcher is running (it keeps running in the tray when its window is closed).
pub fn launcher_running() -> bool {
    if !cfg!(windows) {
        return false;
    }
    craft_anything_platform::Process::new("tasklist")
        .args(["/FI", "IMAGENAME eq artcraft-launcher.exe", "/NH"])
        .timeout(std::time::Duration::from_secs(10))
        .run()
        .is_ok_and(|o| o.stdout_text().to_ascii_lowercase().contains("artcraft-launcher.exe"))
}

/// Close ArtCraft Launcher, so its settings can be changed.
pub fn close_launcher() {
    let _ = craft_anything_platform::Process::new("taskkill")
        .args(["/IM", "artcraft-launcher.exe", "/F"])
        .timeout(std::time::Duration::from_secs(10))
        .run();
    std::thread::sleep(std::time::Duration::from_millis(800));
}

/// `installed.json` with this program recorded as installed, the rest of it kept.
fn record_install(text: &str, exe: &Path, now: i64) -> Result<String, String> {
    let mut doc: serde_json::Value = if text.trim().is_empty() { serde_json::json!({}) } else { serde_json::from_str(text).map_err(|e| e.to_string())? };
    let root = doc.as_object_mut().ok_or("the launcher's list isn't an object")?;
    let apps = root.entry("apps").or_insert_with(|| serde_json::json!({}));
    let apps = apps.as_object_mut().ok_or("the launcher's app list isn't an object")?;
    let path = exe.display().to_string();
    apps.insert(
        APP_ID.into(),
        serde_json::json!({
            "version": base_version(),
            "kind": "PortableZip",
            "path": path,
            "installed_at": now,
            "files": [path],
            "needs_root": false,
        }),
    );
    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}

/// The launcher's settings with this app pinned to [`base_version`], so its "Update" never puts the
/// official release in its place. Already pinned: unchanged. The settings are a string inside the
/// file, so their quotes are escaped there.
fn pin(text: &str) -> Result<String, String> {
    let key = format!(r#"\"{APP_ID}\":"#);
    if text.contains(&key) {
        return Ok(text.to_string());
    }
    let at = text.find("versions:{").ok_or("the launcher's settings have no version choices")? + "versions:{".len();
    let empty = text[at..].starts_with('}');
    let entry = format!(r#"{key}Pinned(\"v{}\"){}"#, base_version(), if empty { "" } else { "," });
    Ok(format!("{}{entry}{}", &text[..at], &text[at..]))
}

/// Show this program in ArtCraft Launcher as an installed app, with its version pinned. The
/// launcher must be closed (see [`launcher_running`]).
pub fn add_to_launcher() -> Result<(), String> {
    let (installed, settings) = launcher_files().ok_or_else(|| tr("ArtCraft Launcher isn't on this computer."))?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let exe = PathBuf::from(exe.display().to_string().trim_start_matches(r"\\?\"));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let list = record_install(&std::fs::read_to_string(&installed).unwrap_or_default(), &exe, now)?;
    craft_anything_platform::write_atomic_private(&installed, list.as_bytes()).map_err(|e| e.message)?;
    // Before the launcher's first start there are no settings yet; it pins nothing until then.
    if let Ok(text) = std::fs::read_to_string(&settings) {
        craft_anything_platform::write_atomic_private(&settings, pin(&text)?.as_bytes()).map_err(|e| e.message)?;
    }
    Ok(())
}

/// The window's own state: what's ticked, and what happened.
#[derive(Clone, Default)]
struct Answers {
    launcher: bool,
    updates: bool,
    started: bool,
    note: Option<(String, bool)>,
}

/// The first-start window of this unofficial version. Not in tests (they run headless).
pub fn show(app: &mut CraftApp, ctx: &egui::Context) {
    if app.headless || app.settings.unofficial.asked {
        return;
    }
    let t = Tokens::get(ctx);
    let id = egui::Id::new("unofficial-welcome");
    let found = launcher_found();
    let mut a: Answers = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    if !a.started {
        a = Answers { launcher: found, updates: true, started: true, note: None };
    }
    let mut done = false;
    egui::Modal::new(id)
        .frame(egui::Frame::new().fill(t.card).corner_radius(t.r_lg()).inner_margin(theme::pad::CARD).stroke(egui::Stroke::new(1.0, t.card_border)))
        .show(ctx, |ui| {
            ui.set_width(520.0_f32.min(ctx.content_rect().width() - 48.0));
            ui.label(theme::heading(tr("An unofficial version of Craft Anything"), theme::SECTION));
            ui.add_space(theme::space::XS);
            ui.add(
                egui::Label::new(
                    RichText::new(tr("This is a modified version by Fiona & Lonco Developer, not the official one. Two quick choices; you can change them later in Settings → General."))
                        .color(t.text_dim),
                )
                .wrap(),
            );
            ui.add_space(theme::space::MD);
            if found {
                widgets::toggle(ui, &mut a.launcher, "Show it in ArtCraft Launcher as an installed app");
                caption(ui, &t, "The launcher lists and opens it. Its version is pinned there, so the launcher never replaces it with the official one.");
                ui.add_space(theme::space::SM);
            }
            widgets::toggle(ui, &mut a.updates, "Tell me when there's a new version");
            caption(
                ui,
                &t,
                &crate::i18n::trf(
                    "Each time it opens it looks at its own page ({repo} on GitHub) and tells you; you download the new version there. It never takes the official one.",
                    &[("repo", crate::updates::OWN_REPO)],
                ),
            );
            if let Some((note, bad)) = &a.note {
                ui.add_space(theme::space::SM);
                ui.add(egui::Label::new(RichText::new(note).color(if *bad { t.warning } else { t.text_dim })).wrap());
            }
            ui.add_space(theme::space::MD);
            ui.horizontal(|ui| {
                if widgets::primary(ui, "Continue").clicked() {
                    if a.launcher && found {
                        if launcher_running() {
                            a.note = Some((tr("ArtCraft Launcher is open. Close it (also from the tray, next to the clock) and press Continue, or use the button."), true));
                        } else {
                            match add_to_launcher() {
                                Ok(()) => done = true,
                                Err(e) => a.note = Some((e, true)),
                            }
                        }
                    } else {
                        done = true;
                    }
                }
                if a.launcher && found && a.note.is_some() && widgets::secondary(ui, None, "Close the launcher and continue").clicked() {
                    close_launcher();
                    match add_to_launcher() {
                        Ok(()) => done = true,
                        Err(e) => a.note = Some((e, true)),
                    }
                }
            });
        });
    if done {
        app.settings.unofficial.asked = true;
        app.settings.unofficial.update_checks = a.updates;
        app.settings_changed();
        ctx.data_mut(|d| d.remove::<Answers>(id));
        if a.updates {
            app.updates.check_quietly(ctx);
        }
    } else {
        ctx.data_mut(|d| d.insert_temp(id, a));
    }
}

fn caption(ui: &mut egui::Ui, t: &Tokens, text: &str) {
    ui.add(egui::Label::new(RichText::new(tr(text)).color(t.text_faint).size(theme::CAPTION)).wrap());
}

/// Settings → General: the same two choices, any time.
pub fn settings_rows(app: &mut CraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(theme::space::MD);
    widgets::caption(ui, "This unofficial version");
    let mut on = app.settings.unofficial.update_checks;
    if widgets::toggle(ui, &mut on, "Tell me when there's a new version").changed() {
        app.settings.unofficial.update_checks = on;
        app.settings_changed();
    }
    if launcher_found() {
        let id = egui::Id::new("unofficial-launcher-note");
        ui.horizontal_wrapped(|ui| {
            if widgets::secondary(ui, None, "Show it in ArtCraft Launcher").clicked() {
                if launcher_running() {
                    close_launcher();
                }
                let note = match add_to_launcher() {
                    Ok(()) => (tr("Done: open ArtCraft Launcher and it's there."), false),
                    Err(e) => (e, true),
                };
                ui.data_mut(|d| d.insert_temp(id, note));
            }
        });
        if let Some((note, bad)) = ui.data(|d| d.get_temp::<(String, bool)>(id)) {
            ui.label(RichText::new(note).color(if bad { t.warning } else { t.text_faint }).size(theme::CAPTION));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_program_is_recorded_and_the_rest_kept() {
        let before = r#"{"apps": {"photocraft": {"version": "0.5.0", "kind": "Msi", "path": "C:/p.exe", "installed_at": 1}}}"#;
        let after: serde_json::Value = serde_json::from_str(&record_install(before, Path::new("C:/Users/a/craft-anything.exe"), 7).unwrap()).unwrap();
        assert_eq!(after["apps"]["photocraft"]["version"], "0.5.0", "another app is kept");
        let me = &after["apps"][APP_ID];
        assert_eq!((me["version"].as_str(), me["kind"].as_str(), me["installed_at"].as_i64()), (Some(base_version()), Some("PortableZip"), Some(7)));
        assert_eq!(me["path"], "C:/Users/a/craft-anything.exe");
        assert!(record_install("", Path::new("x.exe"), 1).is_ok(), "no list yet");
        assert!(record_install("[1]", Path::new("x.exe"), 1).is_err());
    }

    #[test]
    fn the_version_is_pinned_once() {
        let empty = r#"{ "settings": "(view:Grid,versions:{},launcher_version:Stable)", }"#;
        let pinned = pin(empty).unwrap();
        assert!(pinned.contains(&format!(r#"versions:{{\"craft-anything\":Pinned(\"v{}\")}}"#, base_version())), "{pinned}");
        assert_eq!(pin(&pinned).unwrap(), pinned, "already pinned: unchanged");
        let other = r#"{ "settings": "(versions:{\"photocraft\":Nightly},x:1)", }"#;
        let both = pin(other).unwrap();
        assert!(both.contains(r#"\"craft-anything\":Pinned("#) && both.contains(r#",\"photocraft\":Nightly}"#), "{both}");
        assert!(pin("no settings here").is_err());
        assert!(!base_version().contains('-'));
    }
}
