use std::{collections::BTreeMap, time::Duration};

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{Map, Value};
use url::Url;

#[derive(Debug, Clone)]
pub struct SenecClient {
    http: Client,
    base_url: Url,
    post_endpoint: Url,
    chunk_size: usize,
}

impl SenecClient {
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

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub fn post_endpoint(&self) -> &Url {
        &self.post_endpoint
    }

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
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| value.to_string());
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
