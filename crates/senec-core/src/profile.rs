use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::model::MetricProfile;

pub fn load_profile(path: &Path) -> Result<MetricProfile> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read profile file: {}", path.display()))?;

    serde_json::from_str::<MetricProfile>(&content)
        .with_context(|| format!("failed to parse {} as metric profile", path.display()))
}
