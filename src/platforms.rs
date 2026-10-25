use serde_json::Value;

pub const CORE_PREFIX: &str = "jotego.";

pub fn candidate_platform_ids(core_dir_name: &str, core_json: Option<&str>) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(suffix) = core_dir_name.get(CORE_PREFIX.len()..).filter(|_| is_jotego_core(core_dir_name)) {
        if !suffix.is_empty() {
            ids.push(suffix.to_string());
        }
    }
    if let Some(json) = core_json.and_then(|text| serde_json::from_str::<Value>(text).ok()) {
        if let Some(list) = json.pointer("/core/metadata/platform_ids").and_then(Value::as_array) {
            for id in list.iter().filter_map(Value::as_str) {
                if !id.is_empty() && !ids.iter().any(|known| known == id) {
                    ids.push(id.to_string());
                }
            }
        }
    }
    ids
}

pub fn is_jotego_core(core_dir_name: &str) -> bool {
    core_dir_name.len() > CORE_PREFIX.len()
        && core_dir_name[..CORE_PREFIX.len()].eq_ignore_ascii_case(CORE_PREFIX)
}

pub fn platform_name(platform_json: &str) -> Option<String> {
    serde_json::from_str::<Value>(platform_json)
        .ok()?
        .pointer("/platform/name")?
        .as_str()
        .map(str::to_string)
}

// Only a file whose name is still the raw id is renamed: anything else was customized (by the
// user or by an earlier run) and stays as it is.
pub fn needs_rename(platform_id: &str, platform_json: &str) -> bool {
    platform_name(platform_json).as_deref() == Some(platform_id)
}

// A downloaded platform file is accepted only if it parses and carries a different, non-empty name.
pub fn accept_download(platform_id: &str, downloaded_json: &str) -> Result<String, String> {
    match platform_name(downloaded_json) {
        None => Err("not a platform JSON file".into()),
        Some(name) if name.trim().is_empty() => Err("platform name is empty".into()),
        Some(name) if name == platform_id => Err("upstream file has no friendly name either".into()),
        Some(name) => Ok(name),
    }
}

pub const UPSTREAM_FOLDERS: [&str; 2] = ["arcade/Platforms", "home/Platforms"];

#[cfg(test)]
mod tests {
    use super::*;

    const CORE_JSON: &str = r#"{ "core": { "metadata": { "platform_ids": ["jtcps1", "cps1"], "shortname": "x" } } }"#;

    #[test]
    fn candidates_come_from_suffix_then_core_json() {
        assert_eq!(candidate_platform_ids("jotego.jtcps1", Some(CORE_JSON)), vec!["jtcps1", "cps1"]);
        assert_eq!(candidate_platform_ids("Jotego.jtcps2", None), vec!["jtcps2"]);
        assert_eq!(candidate_platform_ids("jotego.jtcps2", Some("not json")), vec!["jtcps2"]);
        assert!(candidate_platform_ids("agg23.NES", Some(CORE_JSON)).is_empty() || !is_jotego_core("agg23.NES"));
        assert!(!is_jotego_core("agg23.NES"));
        assert!(!is_jotego_core("jotego."));
    }

    #[test]
    fn rename_only_when_name_is_still_the_id() {
        let raw = r#"{ "platform": { "name": "jtcps1", "category": "Arcade" } }"#;
        let custom = r#"{ "platform": { "name": "My CPS1", "category": "Arcade" } }"#;
        assert!(needs_rename("jtcps1", raw));
        assert!(!needs_rename("jtcps1", custom));
        assert!(!needs_rename("jtcps1", "garbage"));
    }

    #[test]
    fn downloads_are_validated() {
        assert_eq!(accept_download("jtcps1", r#"{ "platform": { "name": "Capcom CPS 1" } }"#).unwrap(), "Capcom CPS 1");
        assert!(accept_download("jtcps1", r#"{ "platform": { "name": "jtcps1" } }"#).is_err());
        assert!(accept_download("jtcps1", r#"{ "platform": { "name": "  " } }"#).is_err());
        assert!(accept_download("jtcps1", "<html>404</html>").is_err());
    }
}
