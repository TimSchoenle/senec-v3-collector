//! The HTTP conversation with one device.

use std::{collections::BTreeMap, time::Duration};

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{Map, Value};
use url::Url;

/// A configured connection to one SENEC system.
///
/// Cloning shares the connection pool and the cookie jar, so a clone is not a second session.
#[derive(Debug, Clone)]
pub struct SenecClient {
    http: Client,
    base_url: Url,
    post_endpoint: Url,
    chunk_size: usize,
}

impl SenecClient {
    /// Builds a client for the device at `base_url`, posting queries to `post_path` under it.
    ///
    /// `timeout` applies to each request, not to a whole [`Self::query_strings`] call. A
    /// `chunk_size` below one is raised to one. `insecure_tls` accepts whatever certificate the
    /// device presents, which a stock SENEC v3 needs because no public authority signed the one
    /// it has.
    ///
    /// # Errors
    ///
    /// Fails when `post_path` does not resolve against `base_url`, or when the TLS backend cannot
    /// build a client.
    pub fn new(
        base_url: Url,
        post_path: &str,
        timeout: Duration,
        insecure_tls: bool,
        chunk_size: usize,
    ) -> Result<Self> {
        let post_endpoint = resolve_path(&base_url, post_path)?;

        let http = Client::builder()
            .timeout(timeout)
            .cookie_store(true)
            .danger_accept_invalid_certs(insecure_tls)
            .build()
            .context("failed to build reqwest client")?;

        Ok(Self {
            http,
            base_url,
            post_endpoint,
            chunk_size: chunk_size.max(1),
        })
    }

    /// Returns the URL every path is resolved against.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Returns the absolute URL queries are posted to.
    #[must_use]
    pub fn post_endpoint(&self) -> &Url {
        &self.post_endpoint
    }

    /// Fetches `path` from the device and returns the body as text.
    ///
    /// A path without a leading slash gets one, so it resolves against the host and not against
    /// whatever directory the base URL points at.
    ///
    /// # Errors
    ///
    /// Fails when the request does not complete, when the device answers with a non-success
    /// status, or when the body cannot be read to the end.
    pub async fn fetch_text(&self, path: &str) -> Result<String> {
        let url = resolve_path(&self.base_url, path)?;
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .context("GET failed")?
            .error_for_status()
            .context("GET returned error status")?;

        resp.text().await.context("failed to read text body")
    }

    /// Returns the raw string the device holds for each requested key, grouped by object.
    ///
    /// Each object's keys are split into chunks of at most `chunk_size` and sent as one POST per
    /// chunk, so a device that truncates or refuses a large body is handled by lowering that
    /// number. An object with no keys is skipped.
    ///
    /// A key the device leaves out of its answer is absent from the result; a key it refuses is
    /// present, holding the refusal word that [`crate::model::ValueStatus::from_raw`] classifies.
    /// A value that is not a JSON string is rendered as its JSON text.
    ///
    /// # Errors
    ///
    /// Fails on the first chunk whose POST does not complete, answers with a non-success status,
    /// or returns a body that is not JSON. Chunks already merged are dropped with it.
    pub async fn query_strings(
        &self,
        request: &BTreeMap<String, Vec<String>>,
    ) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
        let mut merged = BTreeMap::new();

        for (object, keys) in request {
            if keys.is_empty() {
                continue;
            }

            for chunk in keys.chunks(self.chunk_size) {
                let payload = build_payload(object, chunk);
                let response = self.post_json(&payload).await?;

                let Some(object_values) = response.get(object).and_then(Value::as_object) else {
                    continue;
                };

                let target = merged
                    .entry(object.clone())
                    .or_insert_with(BTreeMap::<String, String>::new);

                for key in chunk {
                    let Some(value) = object_values.get(key) else {
                        continue;
                    };

                    let as_string = value
                        .as_str()
                        .map_or_else(|| value.to_string(), ToOwned::to_owned);
                    target.insert(key.clone(), as_string);
                }
            }
        }

        Ok(merged)
    }

    async fn post_json(&self, payload: &Value) -> Result<Value> {
        self.http
            .post(self.post_endpoint.clone())
            .json(payload)
            .send()
            .await
            .context("POST to lala.cgi failed")?
            .error_for_status()
            .context("lala.cgi returned error status")?
            .json::<Value>()
            .await
            .context("failed to parse lala.cgi response as JSON")
    }
}

/// Builds the body for one object: every key mapped to an empty string, which the device echoes
/// back with the values filled in.
fn build_payload(object: &str, keys: &[String]) -> Value {
    let mut inner = Map::new();
    for key in keys {
        inner.insert(key.clone(), Value::String(String::new()));
    }

    let mut top = Map::new();
    top.insert(object.to_string(), Value::Object(inner));
    Value::Object(top)
}

fn resolve_path(base_url: &Url, path: &str) -> Result<Url> {
    let normalized = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };

    base_url
        .join(&normalized)
        .with_context(|| format!("failed to join URL path: {normalized}"))
}
