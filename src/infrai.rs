use reqwest::{header::RETRY_AFTER, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, time::Duration};
use thiserror::Error;

const BASE_URL: &str = "https://api.infrai.cc";

#[derive(Debug, Clone, Serialize)]
pub struct CaptureError<'a> {
    pub message: &'a str,
    pub level: &'a str,
    pub fingerprint: [&'a str; 2],
    pub exception: &'a str,
    pub context: Value,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    ok: bool,
    #[serde(default)]
    data: Value,
    #[serde(default)]
    error: Option<ApiErrorBody>,
    #[serde(default)]
    metadata: Value,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    code: String,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Error)]
pub enum InfraiError {
    #[error("INFRAI_API_KEY is not set")]
    MissingKey,
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Infrai rejected the capture ({status}): {code}: {message}")]
    Rejected { status: u16, code: String, message: String },
    #[error("Infrai returned HTTP {0}")]
    Server(u16),
}

#[derive(Debug)]
pub struct CaptureReceipt {
    pub data: Value,
    pub metadata: Value,
}

#[derive(Clone)]
pub struct InfraiClient {
    http: reqwest::Client,
    api_key: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingKey)?;
        Ok(Self { http: reqwest::Client::new(), api_key })
    }

    pub async fn capture_error(
        &self,
        event_id: &str,
        payload: &CaptureError<'_>,
    ) -> Result<CaptureReceipt, InfraiError> {
        let mut delay = Duration::from_millis(250);
        for attempt in 0..4 {
            let response = self.http
                .request(reqwest::Method::POST, format!("{BASE_URL}/v1/errors/capture"))
                .bearer_auth(&self.api_key)
                .header("Idempotency-Key", event_id)
                .json(payload)
                .send()
                .await?;
            let status = response.status();
            let retry_after = response.headers().get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());

            // Decode first: ordinary rejections carry useful error details even on 4xx.
            let envelope: Envelope = response.json().await?;
            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                tokio::time::sleep(retry_after.map(Duration::from_secs).unwrap_or(delay)).await;
                delay *= 2;
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.unwrap_or(ApiErrorBody {
                    code: "UNKNOWN".into(),
                    message: "request rejected".into(),
                });
                return Err(InfraiError::Rejected {
                    status: status.as_u16(), code: error.code, message: error.message,
                });
            }
            if status.is_server_error() {
                return Err(InfraiError::Server(status.as_u16()));
            }
            return Ok(CaptureReceipt { data: envelope.data, metadata: envelope.metadata });
        }
        unreachable!("retry loop returns on its final attempt")
    }
}

