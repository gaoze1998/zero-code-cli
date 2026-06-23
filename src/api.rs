use serde::{Deserialize, Serialize};
use std::sync::mpsc::Sender;

use crate::config::Config;

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: DeltaContent,
}

#[derive(Deserialize)]
struct DeltaContent {
    content: Option<String>,
}

/// Build the list of messages to send to the API.
/// Converts the app's conversation history into the API format.
fn build_messages(config: &Config, conversation: &[crate::app::Message]) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    messages.push(ChatMessage {
        role: "system".into(),
        content: config.system_prompt.clone(),
    });

    for msg in conversation {
        let role = match msg.role {
            crate::app::MessageRole::User => "user",
            crate::app::MessageRole::Agent => "assistant",
            crate::app::MessageRole::System => "system",
        };
        messages.push(ChatMessage {
            role: role.into(),
            content: msg.content.clone(),
        });
    }

    messages
}

/// Call the DeepSeek chat completions API with streaming.
/// Sends each token through the provided channel.
pub async fn stream_chat(
    config: &Config,
    conversation: &[crate::app::Message],
    tx: Sender<String>,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let url = format!("{}/v1/chat/completions", config.api_url.trim_end_matches('/'));

    let messages = build_messages(config, conversation);
    let request = ChatRequest {
        model: config.model.clone(),
        messages,
        stream: true,
        max_tokens: config.max_tokens,
        temperature: config.temperature,
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error ({}): {}", status, body));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    loop {
        use futures_util::StreamExt;
        let chunk = stream
            .next()
            .await
            .ok_or_else(|| "Stream ended unexpectedly".to_string())?;
        let chunk = chunk.map_err(|e| format!("Stream read error: {}", e))?;
        let text = String::from_utf8_lossy(&chunk);

        buffer.push_str(&text);
        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if line == "data: [DONE]" {
                return Ok(());
            }

            if let Some(data) = line.strip_prefix("data: ") {
                match serde_json::from_str::<StreamChunk>(data) {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first()
                            && let Some(ref content) = choice.delta.content
                            && tx.send(content.clone()).is_err()
                        {
                            return Ok(());
                        }
                    }
                    Err(_) => continue, // skip unparseable lines
                }
            }
        }
    }
}
