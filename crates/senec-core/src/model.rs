//! The key list a caller polls with, and how the device answers for one key.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The set of keys one device answers for, as written by `senec-v3-discover` and read back by the
/// collector.
///
/// It describes a single system rather than the SENEC v3 as a model, so a profile taken from one
/// device is a starting guess on another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricProfile {
    /// SENEC object name to the keys polled under it.
    ///
    /// An object mapped to an empty list is never polled.
    pub objects: BTreeMap<String, Vec<String>>,
}

/// What the device's answer for one key turned out to be.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ValueStatus {
    /// Anything that is not one of the four refusals, including a value nothing here can decode.
    Ok,
    /// The device answered `VARIABLE_NOT_FOUND`.
    VariableNotFound,
    /// The device answered `FORBIDDEN`.
    Forbidden,
    /// The device answered `OBJECT_NOT_FOUND`.
    ObjectNotFound,
    /// The device answered `MALFORMED_VALUE`.
    MalformedValue,
}

impl ValueStatus {
    /// Classifies one raw value string, matching the refusal words exactly and case-sensitively.
    #[must_use]
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
