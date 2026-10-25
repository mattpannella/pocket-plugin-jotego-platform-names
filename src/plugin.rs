
use extism_pdk::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::platforms;

const CORES_DIR: &str = "pocket/Cores";
const PLATFORMS_DIR: &str = "pocket/Platforms";
const ARCHIVE_DIR: &str = "pocket/Platforms/_archive";
const DEFAULT_BASE_URL: &str = "https://raw.githubusercontent.com/dyreschlock/pocket-platform-images/main/";
// Host config key overriding the base URL (used by the headless test harness).
const BASE_URL_CONFIG: &str = "platform_names_url";
const MISSING_CACHE: &str = "host/missing.json";
const RETRY_AFTER_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Clone, Serialize, ToBytes)]
#[encoding(Json)]
enum PluginMessage {
    #[allow(dead_code)]
    Choice { name: String, query: String, choices: Vec<String> },
    #[allow(dead_code)]
    Text { name: String, query: String },
    Exit,
}

#[derive(Clone, Deserialize, FromBytes)]
#[encoding(Json)]
enum HostMessage {
    #[allow(dead_code)]
    Answer { name: String, value: String },
    Kill,
}

#[host_fn]
extern "ExtismHost" {
    fn open_url(url: &str) -> ();
    fn print_msg(message: &str) -> ();
}

fn println(msg: &str) {
    unsafe {
        let _ = print_msg(&format!("{msg}\n"));
    }
}

fn base_url() -> String {
    config::get(BASE_URL_CONFIG).ok().flatten().filter(|s| !s.is_empty()).unwrap_or_else(|| DEFAULT_BASE_URL.into())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn load_missing() -> HashMap<String, u64> {
    fs::read_to_string(MISSING_CACHE).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_missing(missing: &HashMap<String, u64>) {
    if let Ok(json) = serde_json::to_string(missing) {
        let _ = fs::write(MISSING_CACHE, json);
    }
}

fn jotego_core_dirs() -> Vec<String> {
    let mut dirs: Vec<String> = fs::read_dir(CORES_DIR)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| platforms::is_jotego_core(name))
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs
}

// Platforms/<id>.json, else the archived copy, else nothing (pupdate never creates a new file).
fn platform_file(id: &str) -> Option<PathBuf> {
    [PLATFORMS_DIR, ARCHIVE_DIR]
        .iter()
        .map(|dir| Path::new(dir).join(format!("{id}.json")))
        .find(|p| p.is_file())
}

enum Fetch {
    Found(String),
    Missing,
    Failed(String),
}

fn fetch_upstream(id: &str) -> Fetch {
    let base = base_url();
    for folder in platforms::UPSTREAM_FOLDERS {
        let url = format!("{base}{folder}/{id}.json");
        let req = HttpRequest::new(&url).with_header("User-Agent", "pocket-plugin-jotego-platform-names/0.1");
        match http::request::<()>(&req, None) {
            Ok(res) if res.status_code() == 200 => return Fetch::Found(String::from_utf8_lossy(&res.body()).into_owned()),
            Ok(res) if res.status_code() == 404 => continue,
            Ok(res) => return Fetch::Failed(format!("{url}: HTTP {}", res.status_code())),
            Err(e) => return Fetch::Failed(format!("{url}: {e}")),
        }
    }
    Fetch::Missing
}

fn replace_file(path: &Path, contents: &str) -> Result<(), String> {
    let part = path.with_extension("json.part");
    fs::write(&part, contents).map_err(|e| format!("could not write {}: {e}", part.display()))?;
    fs::rename(&part, path).map_err(|e| format!("could not replace {}: {e}", path.display()))
}

#[plugin_fn]
pub fn start() -> FnResult<PluginMessage> {
    println("Jotego platform names");
    let cores = jotego_core_dirs();
    if cores.is_empty() {
        println("No Jotego cores installed in Cores/; nothing to do.");
        return Ok(PluginMessage::Exit);
    }

    let mut missing = load_missing();
    let mut seen: Vec<String> = Vec::new();
    let (mut renamed, mut skipped, mut failed) = (0, 0, 0);

    for core in &cores {
        let core_json = fs::read_to_string(Path::new(CORES_DIR).join(core).join("core.json")).ok();
        for id in platforms::candidate_platform_ids(core, core_json.as_deref()) {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id.clone());

            let Some(path) = platform_file(&id) else { continue };
            let Ok(current) = fs::read_to_string(&path) else { continue };
            if !platforms::needs_rename(&id, &current) {
                skipped += 1;
                continue;
            }
            if missing.get(&id).map(|t| now().saturating_sub(*t) < RETRY_AFTER_SECS).unwrap_or(false) {
                println(&format!("{id}: no friendly name upstream (checked recently); skipping."));
                skipped += 1;
                continue;
            }

            match fetch_upstream(&id) {
                Fetch::Found(json) => match platforms::accept_download(&id, &json) {
                    Ok(name) => match replace_file(&path, &json) {
                        Ok(()) => {
                            println(&format!("{id}: renamed to \"{name}\" ({}).", path.strip_prefix("pocket/").unwrap_or(&path).display()));
                            renamed += 1;
                            missing.remove(&id);
                        }
                        Err(e) => {
                            println(&format!("{id}: {e}"));
                            failed += 1;
                        }
                    },
                    Err(e) => {
                        println(&format!("{id}: upstream file rejected ({e}); left as is."));
                        missing.insert(id.clone(), now());
                        skipped += 1;
                    }
                },
                Fetch::Missing => {
                    println(&format!("{id}: no friendly name upstream; left as is."));
                    missing.insert(id.clone(), now());
                    skipped += 1;
                }
                Fetch::Failed(e) => {
                    println(&format!("{id}: download failed ({e})."));
                    failed += 1;
                }
            }
        }
    }

    save_missing(&missing);
    println(&format!("Done: {renamed} renamed, {skipped} left as is, {failed} failed."));
    Ok(PluginMessage::Exit)
}

#[plugin_fn]
pub fn handle_response(_input: HostMessage) -> FnResult<PluginMessage> {
    Ok(PluginMessage::Exit)
}
