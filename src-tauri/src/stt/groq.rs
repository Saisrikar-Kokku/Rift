use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Debug, Deserialize)]
struct GroqTranscriptionResponse {
    text: String,
}

#[derive(Debug, Clone, Default)]
pub struct GroqRateLimitInfo {
    pub limit_requests: Option<u64>,
    pub remaining_requests: Option<u64>,
    pub reset_requests: Option<String>,
    pub last_updated: Option<std::time::Instant>,
}

#[derive(Clone)]
pub struct GroqClient {
    client: Client,
    rate_limits: Arc<Mutex<GroqRateLimitInfo>>,
}

impl GroqClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .tcp_nodelay(true)
                .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
                .pool_idle_timeout(Some(std::time::Duration::from_secs(300)))
                .pool_max_idle_per_host(8)
                .timeout(std::time::Duration::from_secs(45))
                .build()
                .unwrap_or_default(),
            rate_limits: Arc::new(Mutex::new(GroqRateLimitInfo::default())),
        }
    }

    /// Pre-warms the TCP and TLS 1.3 connection to Groq in the background during PTT press
    /// so the audio request can be transmitted instantly on key release.
    pub fn prewarm(&self, api_key: &str) {
        if api_key.trim().is_empty() {
            return;
        }
        let client = self.client.clone();
        let key = api_key.trim().to_string();
        tauri::async_runtime::spawn(async move {
            let _ = client
                .get("https://api.groq.com/openai/v1/models")
                .header("Authorization", format!("Bearer {}", key))
                .send()
                .await;
        });
    }

    pub fn get_rate_limit_info(&self) -> GroqRateLimitInfo {
        self.rate_limits.lock().map(|l| l.clone()).unwrap_or_default()
    }

    pub async fn transcribe(
        &self,
        api_key: &str,
        wav_bytes: Vec<u8>,
        model: &str,
        prompt: Option<&str>,
        language: Option<&str>,
    ) -> Result<String, String> {
        let url = "https://api.groq.com/openai/v1/audio/transcriptions";

        let file_part = Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| e.to_string())?;

        let mut form = Form::new()
            .part("file", file_part)
            .text("model", model.to_string())
            .text("response_format", "json")
            .text("temperature", "0.0");

        if let Some(p) = prompt {
            if !p.trim().is_empty() {
                form = form.text("prompt", p.trim().to_string());
            }
        }

        if let Some(lang) = language {
            if !lang.trim().is_empty() {
                form = form.text("language", lang.trim().to_string());
            }
        }

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key.trim()))
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Network request to Groq failed: {}", e))?;

        // Capture live rate limit headers from Groq
        let remaining_reqs = response
            .headers()
            .get("x-ratelimit-remaining-requests")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        let limit_reqs = response
            .headers()
            .get("x-ratelimit-limit-requests")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        let reset_reqs = response
            .headers()
            .get("x-ratelimit-reset-requests")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        if remaining_reqs.is_some() || limit_reqs.is_some() {
            if let Ok(mut lock) = self.rate_limits.lock() {
                if let Some(r) = remaining_reqs {
                    lock.remaining_requests = Some(r);
                }
                if let Some(l) = limit_reqs {
                    lock.limit_requests = Some(l);
                }
                if let Some(res) = reset_reqs {
                    lock.reset_requests = Some(res);
                }
                lock.last_updated = Some(std::time::Instant::now());
            }
        }

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(format!("Groq API error ({}): {}", status, err_body));
        }

        let result: GroqTranscriptionResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

        Ok(result.text)
    }
}
