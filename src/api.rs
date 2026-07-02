use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::mpsc::Sender;

use crate::app::AgentEvent;
use crate::config::Config;
use crate::tools::ToolDefinition;

#[derive(Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallMsg>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ToolCallMsg {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: FunctionCall,
}

#[derive(Serialize, Clone)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
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
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Deserialize)]
struct ToolCallDelta {
    index: usize,
    id: Option<String>,
    function: Option<FunctionDelta>,
}

#[derive(Deserialize)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

/// Accumulator for streaming tool call fragments.
struct AccToolCall {
    id: String,
    name: String,
    arguments: String,
    started: bool,
}

/// Response structs for non-streaming chat completion.
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatResponseChoice>,
}

#[derive(Deserialize)]
struct ChatResponseChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
}

/// Non-streaming chat completion for summarization tasks.
pub async fn chat_sync(
    config: &Config,
    messages: &[ChatMessage],
    max_tokens: u32,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let url = format!(
        "{}/v1/chat/completions",
        config.api_url.trim_end_matches('/')
    );

    let request = ChatRequest {
        model: config.model.clone(),
        messages: messages.to_vec(),
        stream: false,
        max_tokens,
        temperature: config.temperature,
        tools: None,
        tool_choice: None,
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
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error ({}): {}", status, body));
    }

    let chat_response: ChatResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    chat_response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| "Empty response from API".to_string())
}

/// Build the list of messages to send to the API.
fn build_messages(config: &Config, conversation: &[crate::app::Message]) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    messages.push(ChatMessage {
        role: "system".into(),
        content: Some(config.system_prompt.clone()),
        tool_calls: None,
        tool_call_id: None,
    });

    for msg in conversation {
        match msg.role {
            crate::app::MessageRole::Tool => {
                // Tool result message
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: Some(msg.content.clone()),
                    tool_calls: None,
                    tool_call_id: msg.tool_call_id.clone(),
                });
            }
            _ => {
                let role = match msg.role {
                    crate::app::MessageRole::User => "user",
                    crate::app::MessageRole::Agent => "assistant",
                    crate::app::MessageRole::System => "system",
                    crate::app::MessageRole::Tool => unreachable!(),
                };
                let (content, tool_calls) = if msg.role == crate::app::MessageRole::Agent
                    && let Some(ref tcs) = msg.tool_calls
                {
                    let tcs: Vec<ToolCallMsg> = tcs
                        .iter()
                        .map(|tc| ToolCallMsg {
                            id: tc.id.clone(),
                            call_type: "function".into(),
                            function: FunctionCall {
                                name: tc.name.clone(),
                                arguments: tc.arguments.clone(),
                            },
                        })
                        .collect();
                    // content must be null when tool_calls are present,
                    // but include it if non-empty (some APIs accept both)
                    let content = if msg.content.is_empty() {
                        None
                    } else {
                        Some(msg.content.clone())
                    };
                    (content, Some(tcs))
                } else {
                    (Some(msg.content.clone()), None)
                };

                messages.push(ChatMessage {
                    role: role.into(),
                    content,
                    tool_calls,
                    tool_call_id: None,
                });
            }
        }
    }

    messages
}

/// Information about a completed tool call, returned to the agent loop.
pub struct CompletedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Call the DeepSeek chat completions API with streaming and tool support.
/// Sends AgentEvent variants through the channel and returns completed tool calls.
pub async fn stream_chat(
    config: &Config,
    conversation: &[crate::app::Message],
    tools: Option<Vec<ToolDefinition>>,
    event_tx: Sender<AgentEvent>,
) -> Result<Vec<CompletedToolCall>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let url = format!(
        "{}/v1/chat/completions",
        config.api_url.trim_end_matches('/')
    );

    let messages = build_messages(config, conversation);
    crate::debug!(
        "Sending request to {} with model {}",
        url,
        config.model
    );
    crate::debug!("{} messages in conversation", messages.len());

    let tool_choice = if tools.is_some() {
        Some(serde_json::json!("auto"))
    } else {
        None
    };

    let request = ChatRequest {
        model: config.model.clone(),
        messages,
        stream: true,
        max_tokens: config.max_tokens,
        temperature: config.temperature,
        tools,
        tool_choice,
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

    // Tool call accumulation by index
    let mut acc_tool_calls: Vec<AccToolCall> = Vec::new();

    let collect_completed = |acc: &[AccToolCall]| -> Vec<CompletedToolCall> {
        acc.iter()
            .filter(|tc| !tc.id.is_empty() && !tc.name.is_empty())
            .map(|tc| CompletedToolCall {
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments: tc.arguments.clone(),
            })
            .collect()
    };

    loop {
        use futures_util::StreamExt;
        let chunk_result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            stream.next(),
        )
        .await;
        let chunk = match chunk_result {
            Ok(Some(result)) => result,
            Ok(None) => {
                crate::debug!("Stream ended without [DONE], {} tokens received", token_count);
                return Ok(collect_completed(&acc_tool_calls));
            }
            Err(_elapsed) => {
                crate::debug!("Stream read timed out after 30s, {} tokens received", token_count);
                return Ok(collect_completed(&acc_tool_calls));
            }
        };
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
                return Ok(collect_completed(&acc_tool_calls));
            }

            if let Some(data) = line.strip_prefix("data: ") {
                match serde_json::from_str::<StreamChunk>(data) {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first() {
                            // Handle text content
                            let delta = &choice.delta;
                            let token = delta
                                .content
                                .as_ref()
                                .or(delta.reasoning_content.as_ref());

                            if let Some(ref token) = token
                                && !token.is_empty()
                            {
                                token_count += 1;
                                if token_count <= 3 {
                                    crate::debug!(
                                        "Token {}: {:?}",
                                        token_count,
                                        token
                                    );
                                }
                                if event_tx
                                    .send(AgentEvent::Token(token.to_string()))
                                    .is_err()
                                {
                                    crate::debug!(
                                        "Receiver dropped, stopping stream"
                                    );
                                    return Ok(Vec::new());
                                }
                            }

                            // Handle tool_calls
                            if let Some(ref tc_deltas) = delta.tool_calls {
                                for tc_delta in tc_deltas {
                                    let index = tc_delta.index;

                                    // Ensure accumulator has this index
                                    while acc_tool_calls.len() <= index {
                                        acc_tool_calls.push(AccToolCall {
                                            id: String::new(),
                                            name: String::new(),
                                            arguments: String::new(),
                                            started: false,
                                        });
                                    }

                                    let acc = &mut acc_tool_calls[index];

                                    // Update id if present
                                    if let Some(ref id) = tc_delta.id {
                                        acc.id = id.clone();
                                    }

                                    // Update function name/arguments if present
                                    if let Some(ref func) = tc_delta.function {
                                        if let Some(ref name) = func.name
                                            && !name.is_empty()
                                        {
                                            acc.name = name.clone();
                                        }
                                        if let Some(ref args) = func.arguments
                                            && !args.is_empty()
                                        {
                                            acc.arguments.push_str(args);
                                            if event_tx
                                                .send(AgentEvent::ToolCallArg {
                                                    id: acc.id.clone(),
                                                    args: args.clone(),
                                                })
                                                .is_err()
                                            {
                                                return Ok(Vec::new());
                                            }
                                        }
                                    }

                                    // Send ToolCallStart once we have id and name
                                    if !acc.started
                                        && !acc.id.is_empty()
                                        && !acc.name.is_empty()
                                    {
                                        acc.started = true;
                                        if event_tx
                                            .send(AgentEvent::ToolCallStart {
                                                id: acc.id.clone(),
                                                name: acc.name.clone(),
                                            })
                                            .is_err()
                                        {
                                            return Ok(Vec::new());
                                        }
                                    }
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
    fn test_delta_parses_tool_call() {
        let json = r#"{"choices":[{"delta":{"content":null,"reasoning_content":null,"tool_calls":[{"index":0,"id":"call_123","function":{"name":"read_file","arguments":"{\"path\":\"src/main.rs\"}"}}]}}]}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        let delta = &chunk.choices[0].delta;
        assert!(delta.content.is_none());
        let tc = &delta.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc.index, 0);
        assert_eq!(tc.id.as_deref(), Some("call_123"));
        let func = tc.function.as_ref().unwrap();
        assert_eq!(func.name.as_deref(), Some("read_file"));
        assert_eq!(
            func.arguments.as_deref(),
            Some("{\"path\":\"src/main.rs\"}")
        );
    }

    #[test]
    fn test_build_messages_includes_tool_messages() {
        let config = Config::load();
        let conversation = vec![
            crate::app::Message {
                role: crate::app::MessageRole::User,
                content: "read file".into(),
                tool_calls: None,
                tool_call_id: None,
                tool_result_error: false,
            },
            crate::app::Message {
                role: crate::app::MessageRole::Tool,
                content: "file contents here".into(),
                tool_calls: None,
                tool_call_id: Some("call_1".into()),
                tool_result_error: false,
            },
        ];

        let messages = build_messages(&config, &conversation);
        // system + user + tool
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "tool");
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn test_build_messages_handles_agent_tool_calls() {
        let config = Config::load();
        let conversation = vec![crate::app::Message {
            role: crate::app::MessageRole::Agent,
            content: "Let me check".into(),
            tool_calls: Some(vec![crate::app::ToolCall {
                id: "call_1".into(),
                name: "ls".into(),
                arguments: r#"{"path":"src"}"#.into(),
            }]),
            tool_call_id: None,
            tool_result_error: false,
        }];

        let messages = build_messages(&config, &conversation);
        assert_eq!(messages.len(), 2); // system + assistant
        assert_eq!(messages[1].role, "assistant");
        let tcs = messages[1].tool_calls.as_ref().unwrap();
        assert_eq!(tcs.len(), 1);
        assert_eq!(tcs[0].id, "call_1");
        assert_eq!(tcs[0].function.name, "ls");
    }

    #[test]
    #[ignore] // Requires DEEPSEEK_API_KEY env var
    fn test_live_api_streaming() {
        let config = Config::load();
        if config.api_key.is_empty() {
            eprintln!("Skipping: DEEPSEEK_API_KEY not set");
            return;
        }

        let conversation = vec![crate::app::Message {
            role: crate::app::MessageRole::User,
            content: "say hi".into(),
            tool_calls: None,
            tool_call_id: None,
            tool_result_error: false,
        }];

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let (tx, rx) = mpsc::channel::<AgentEvent>();

        rt.block_on(async {
            match stream_chat(&config, &conversation, None, tx).await {
                Ok(_) => eprintln!("OK: stream completed"),
                Err(e) => panic!("API call failed: {}", e),
            }
        });

        let mut all_tokens = String::new();
        while let Ok(event) = rx.try_recv() {
            if let AgentEvent::Token(t) = event {
                all_tokens.push_str(&t);
            }
        }
        assert!(
            !all_tokens.is_empty(),
            "Expected non-empty response, got nothing"
        );
    }
}
