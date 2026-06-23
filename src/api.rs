use serde::{Deserialize, Serialize};
use std::sync::mpsc::Sender;
use std::time::Duration;

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
    reasoning_content: Option<String>,
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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let url = format!("{}/v1/chat/completions", config.api_url.trim_end_matches('/'));

    let messages = build_messages(config, conversation);
    crate::debug!("Sending request to {} with model {}", url, config.model);
    crate::debug!("{} messages in conversation", messages.len());

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

    let status = response.status();
    crate::debug!("Response status: {}", status);

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        crate::debug!("Error body: {}", body);
        return Err(format!("API error ({}): {}", status, body));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut token_count: u32 = 0;

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
                crate::debug!("Stream complete, {} tokens received", token_count);
                return Ok(());
            }

            if let Some(data) = line.strip_prefix("data: ") {
                match serde_json::from_str::<StreamChunk>(data) {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first() {
                            let token = choice.delta.content.as_ref().or(
                                choice.delta.reasoning_content.as_ref(),
                            );
                            if let Some(ref token) = token
                                && !token.is_empty()
                            {
                                token_count += 1;
                                if token_count <= 3 {
                                    crate::debug!("Token {}: {:?}", token_count, token);
                                }
                                if tx.send(token.to_string()).is_err() {
                                    crate::debug!("Receiver dropped, stopping stream");
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_delta_parses_reasoning_content() {
        let json = r#"{"choices":[{"delta":{"content":null,"reasoning_content":"hello"}}]}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        let delta = &chunk.choices[0].delta;
        assert!(delta.content.is_none());
        assert_eq!(delta.reasoning_content.as_deref(), Some("hello"));
    }

    #[test]
    fn test_delta_parses_content() {
        let json = r#"{"choices":[{"delta":{"content":"world","reasoning_content":null}}]}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        let delta = &chunk.choices[0].delta;
        assert_eq!(delta.content.as_deref(), Some("world"));
        assert!(delta.reasoning_content.is_none());
    }

    #[test]
    fn test_delta_parses_both_null() {
        let json = r#"{"choices":[{"delta":{"content":null,"reasoning_content":null}}]}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        let delta = &chunk.choices[0].delta;
        assert!(delta.content.is_none());
        assert!(delta.reasoning_content.is_none());
    }

    #[test]
    #[ignore] // Requires DEEPSEEK_API_KEY env var
    fn test_live_api_streaming() {
        let config = Config::load();
        if config.api_key.is_empty() {
            eprintln!("Skipping: DEEPSEEK_API_KEY not set");
            return;
        }

        let conversation = vec![
            crate::app::Message {
                role: crate::app::MessageRole::User,
                content: "say hi".into(),
            },
        ];

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let (tx, rx) = mpsc::channel::<String>();

        rt.block_on(async {
            match stream_chat(&config, &conversation, tx).await {
                Ok(()) => eprintln!("OK: stream completed"),
                Err(e) => panic!("API call failed: {}", e),
            }
        });

        let mut all_tokens = String::new();
        while let Ok(token) = rx.try_recv() {
            all_tokens.push_str(&token);
        }
        assert!(!all_tokens.is_empty(), "Expected non-empty response, got nothing");
    }
}
