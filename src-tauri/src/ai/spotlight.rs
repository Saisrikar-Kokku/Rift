use serde_json::{json, Value};
use crate::storage::credentials::CredentialsProvider;

/// Queries Groq AI chat completion STRICTLY using the Groq API key and Groq llama-3.3-70b-versatile model.
/// OpenRouter or premium models are NEVER contacted for Spotlight AI queries.
pub async fn query_groq_spotlight(prompt: &str, context: Option<&str>) -> Result<String, String> {
    let api_key = CredentialsProvider::get_api_key()
        .ok_or_else(|| "Groq API key not found. Please add your Groq API key in Settings -> Models.".to_string())?;

    let trimmed_key = api_key.trim();
    if trimmed_key.is_empty() {
        return Err("Groq API key is empty. Please configure your Groq key in Settings -> Models.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let system_content = if let Some(ctx) = context {
        format!(
            "You are Rift Spotlight Voice AI, an ultra-fast desktop assistant powered by Groq Llama 3.3.\n\
             Answer the user's question directly, clearly, and concisely using markdown formatting.\n\
             Below is context from the user's active window or current selection:\n\
             ---\n\
             {}\n\
             ---",
            ctx.trim()
        )
    } else {
        "You are Rift Spotlight Voice AI, an ultra-fast desktop assistant powered by Groq Llama 3.3.\n\
         Answer the user's question directly, clearly, and concisely using clean markdown formatting.\n\
         Be direct and avoid unnecessary fluff."
            .to_string()
    };

    let models = ["qwen/qwen3.8-27b", "groq/compound-mini", "openai/gpt-oss-20b"];
    let mut last_err = String::new();

    for model in &models {
        let req_body = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": &system_content },
                { "role": "user", "content": prompt.trim() }
            ],
            "temperature": 0.2,
            "max_tokens": 2048
        });

        let resp = client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .bearer_auth(trimmed_key)
            .header("User-Agent", "Rift-App/1.0")
            .json(&req_body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let data: Value = r
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

                if let Some(content) = data["choices"][0]["message"]["content"].as_str() {
                    return Ok(content.trim().to_string());
                }
            }
            Ok(r) => {
                let status = r.status();
                let err_body = r.text().await.unwrap_or_default();
                last_err = format!("Groq API error ({}) with {}: {}", status, model, err_body);
            }
            Err(e) => {
                last_err = format!("Network request to Groq failed with {}: {}", model, e);
            }
        }
    }

    Err(last_err)
}

