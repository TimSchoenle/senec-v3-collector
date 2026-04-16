use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use regex::Regex;

use senec_core::{
    client::SenecClient,
    model::{MetricProfile, ValueStatus},
};

pub async fn discover(client: &SenecClient) -> Result<MetricProfile> {
    let candidates = build_candidates(client).await?;

    let request_map: BTreeMap<String, Vec<String>> = candidates
        .iter()
        .map(|(object, keys)| (object.clone(), keys.iter().cloned().collect()))
        .collect();

    let responses = client.query_strings(&request_map).await?;

    let mut objects = BTreeMap::new();
    for (object, candidate_keys) in candidates {
        let Some(values) = responses.get(&object) else {
            continue;
        };

        let keys: Vec<String> = candidate_keys
            .into_iter()
            .filter(|key| {
                values
                    .get(key)
                    .is_some_and(|value| ValueStatus::from_raw(value) == ValueStatus::Ok)
            })
            .collect();

        if !keys.is_empty() {
            objects.insert(object, keys);
        }
    }

    Ok(MetricProfile { objects })
}

async fn build_candidates(client: &SenecClient) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let senec_js = client
        .fetch_text("/js/senec.min.js")
        .await
        .context("failed to load /js/senec.min.js")?;

    let mut objects = extract_objects(&senec_js)?;
    for object in extract_property_objects(&senec_js)? {
        if !objects.contains(&object) {
            objects.push(object);
        }
    }
    for object in extract_action_objects(&senec_js)? {
        if !objects.contains(&object) {
            objects.push(object);
        }
    }
    objects.sort();

    let mut candidates: BTreeMap<String, BTreeSet<String>> = objects
        .iter()
        .map(|o| (o.clone(), BTreeSet::new()))
        .collect();

    for (object, key) in extract_properties(&senec_js)? {
        candidates.entry(object).or_default().insert(key);
    }

    for (object, key) in extract_action_keys(&senec_js)? {
        candidates.entry(object).or_default().insert(key);
    }

    for id in extract_ids(&senec_js)? {
        for object in &objects {
            if id.starts_with(object) && id.len() > object.len() {
                let key = id[object.len()..].to_string();
                candidates.entry(object.clone()).or_default().insert(key);
                break;
            }
        }
    }

    let mut html_paths = extract_html_paths(&senec_js)?;
    html_paths.insert("/".to_string());

    for path in html_paths {
        let html = match client.fetch_text(&path).await {
            Ok(html) => html,
            Err(err) => {
                tracing::warn!(%path, %err, "skipping HTML asset during discovery");
                continue;
            }
        };

        for id in extract_ids(&html)? {
            for object in &objects {
                if id.starts_with(object) && id.len() > object.len() {
                    let key = id[object.len()..].to_string();
                    candidates.entry(object.clone()).or_default().insert(key);
                    break;
                }
            }
        }
    }

    Ok(candidates)
}

fn extract_objects(js: &str) -> Result<Vec<String>> {
    let re = Regex::new(r#"add_json_object\("([A-Z0-9_]+)"\)"#)?;
    let mut objects: BTreeSet<String> = BTreeSet::new();

    for cap in re.captures_iter(js) {
        objects.insert(cap[1].to_string());
    }

    Ok(objects.into_iter().collect())
}

fn extract_property_objects(js: &str) -> Result<Vec<String>> {
    let re = Regex::new(r#"add_property_to_object\("([A-Z0-9_]+)","([A-Z0-9_]+)""#)?;
    let mut objects: BTreeSet<String> = BTreeSet::new();

    for cap in re.captures_iter(js) {
        objects.insert(cap[1].to_string());
    }

    Ok(objects.into_iter().collect())
}

fn extract_properties(js: &str) -> Result<Vec<(String, String)>> {
    let re = Regex::new(r#"add_property_to_object\("([A-Z0-9_]+)","([A-Z0-9_]+)""#)?;
    Ok(re
        .captures_iter(js)
        .map(|cap| (cap[1].to_string(), cap[2].to_string()))
        .collect())
}

fn extract_action_objects(js: &str) -> Result<Vec<String>> {
    let re = Regex::new(
        r#"(?:handleButtonClick|handleSelectUpdate|handleCheckBoxUpdate|handleTextAreaUpdate|handleTextAreaUpdateArray|handleCheckBoxUpdateArray)\("([A-Z0-9_]+)","([A-Z0-9_]+)""#,
    )?;
    let mut objects: BTreeSet<String> = BTreeSet::new();

    for cap in re.captures_iter(js) {
        objects.insert(cap[1].to_string());
    }

    Ok(objects.into_iter().collect())
}

fn extract_action_keys(js: &str) -> Result<Vec<(String, String)>> {
    let re = Regex::new(
        r#"(?:handleButtonClick|handleSelectUpdate|handleCheckBoxUpdate|handleTextAreaUpdate|handleTextAreaUpdateArray|handleCheckBoxUpdateArray)\("([A-Z0-9_]+)","([A-Z0-9_]+)""#,
    )?;
    Ok(re
        .captures_iter(js)
        .map(|cap| (cap[1].to_string(), cap[2].to_string()))
        .collect())
}

fn extract_html_paths(js: &str) -> Result<BTreeSet<String>> {
    let re = Regex::new(r#""(\.?/?[A-Za-z0-9_./-]+\.html)""#)?;
    let mut paths = BTreeSet::new();

    for cap in re.captures_iter(js) {
        let raw = &cap[1];
        let normalized = normalize_path(raw);
        paths.insert(normalized);
    }

    Ok(paths)
}

fn extract_ids(html: &str) -> Result<Vec<String>> {
    let re = Regex::new(r#"id\s*=\s*(?:"([^"]+)"|([^\s>]+))"#)?;
    let mut ids = Vec::new();

    for cap in re.captures_iter(html) {
        let id = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str())
            .unwrap_or_default()
            .trim_matches(['"', '\'', ' ']);

        if !id.is_empty() {
            ids.push(id.to_string());
        }
    }

    Ok(ids)
}

fn normalize_path(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }

    if let Some(stripped) = path.strip_prefix("./") {
        return format!("/{stripped}");
    }

    if path.starts_with('/') {
        return path.to_string();
    }

    format!("/{path}")
}

#[cfg(test)]
mod tests {
    use super::{extract_action_keys, extract_objects, normalize_path};

    #[test]
    fn normalizes_paths() {
        assert_eq!(normalize_path("./abc.html"), "/abc.html");
        assert_eq!(normalize_path("abc.html"), "/abc.html");
        assert_eq!(normalize_path("/abc.html"), "/abc.html");
    }

    #[test]
    fn extracts_objects_from_js() {
        let js = r#"x.add_json_object("ENERGY");x.add_json_object("BMS");"#;
        let objects = extract_objects(js).expect("regex should compile");
        assert_eq!(objects, vec!["BMS", "ENERGY"]);
    }

    #[test]
    fn extracts_action_keys_from_js() {
        let js = r#"x.handleButtonClick("BMS","WIZARD_START");"#;
        let keys = extract_action_keys(js).expect("regex should compile");
        assert_eq!(keys, vec![("BMS".to_string(), "WIZARD_START".to_string())]);
    }
}
