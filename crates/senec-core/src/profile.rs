//! Reading a metric profile off disk.

use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::model::MetricProfile;

/// Reads the profile at `path`.
///
/// # Errors
///
/// Fails when the file cannot be read, or when its contents are not a JSON object with an
/// `objects` map in it.
pub fn load_profile(path: &Path) -> Result<MetricProfile> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read profile file: {}", path.display()))?;

    serde_json::from_str::<MetricProfile>(&content)
        .with_context(|| format!("failed to parse {} as metric profile", path.display()))
}
