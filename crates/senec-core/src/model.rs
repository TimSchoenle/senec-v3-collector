use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricProfile {
    pub objects: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ValueStatus {
    Ok,
    VariableNotFound,
    Forbidden,
    ObjectNotFound,
    MalformedValue,
}

impl ValueStatus {
    pub fn from_raw(value: &str) -> Self {
        match value {
            "VARIABLE_NOT_FOUND" => Self::VariableNotFound,
            "FORBIDDEN" => Self::Forbidden,
            "OBJECT_NOT_FOUND" => Self::ObjectNotFound,
            "MALFORMED_VALUE" => Self::MalformedValue,
            _ => Self::Ok,
        }
    }
}
