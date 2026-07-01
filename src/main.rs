#![forbid(unsafe_code)]

mod api;
mod app;
mod config;
mod logger;
mod tools;
mod ui;

use std::io::{self, stdout};
use std::sync::mpsc;
use std::time::Duration;

use app::{AgentEvent, App, Mode};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

fn main() -> io::Result<()> {
    let log_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".zero-code-cli")
        .join("debug.log");
    let _ = logger::init(&log_path);

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");
    let mut app = App::new();

    let (event_tx, event_rx) = mpsc::channel::<AgentEvent>();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // Drive the tokio runtime so spawned async tasks make progress
        rt.block_on(tokio::task::yield_now());

        // Drain agent events
        while let Ok(event) = event_rx.try_recv() {
            app.handle_agent_event(event);
        }

        // Poll for input
        if event::poll(Duration::from_millis(16)).unwrap_or(false) {
            match event::read()? {
                Event::Key(key) => {
                    handle_key(&mut app, key, &rt, &event_tx);
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if app.should_quit {
            rt.shutdown_background();
            break;
        }
    }

    Ok(())
}

fn handle_key(
    app: &mut App,
    key: KeyEvent,
    rt: &tokio::runtime::Runtime,
    event_tx: &mpsc::Sender<AgentEvent>,
) {
    // Ctrl+C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }

    // Ctrl+D quits on empty input
    if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if app.input.is_empty() {
            app.should_quit = true;
        }
        return;
    }

    match key.code {
        KeyCode::Tab => {
            if !app.agent_active {
                let next = match app.current_mode {
                    Mode::Plan => Mode::Build,
                    Mode::Build => Mode::Plan,
                };
                app.switch_mode(next);
            }
        }
        KeyCode::Enter => {
            let input = app.input.trim().to_string();
            if input.starts_with('/') && app.handle_slash_command(&input) {
                // slash command handled
            } else if let Some(_msg) = app.send_message() {
                let config = app.config.clone();
                let mode = app.current_mode;
                let plan_artifact = app.plan_artifact.clone();
                let conversation: Vec<app::Message> = app
                    .active_messages()
                    .iter()
                    .map(|m| app::Message {
                        role: m.role,
                        content: m.content.clone(),
                        tool_calls: m.tool_calls.clone(),
                        tool_call_id: m.tool_call_id.clone(),
                        tool_result_error: m.tool_result_error,
                    })
                    .collect();
                let tx = event_tx.clone();
                rt.spawn(async move {
                    agent_loop(&config, &conversation, mode, plan_artifact, tx).await;
                });
            }
        }
        KeyCode::Backspace => {
            if !app.agent_active {
                app.delete_char();
            }
        }
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if !app.agent_active {
                app.delete_word();
            }
        }
        KeyCode::Left => {
            app.move_cursor_left();
        }
        KeyCode::Right => {
            app.move_cursor_right();
        }
        KeyCode::Home => {
            app.move_cursor_home();
        }
        KeyCode::End => {
            app.move_cursor_end();
        }
        KeyCode::Up => {
            app.scroll_up();
        }
        KeyCode::Down => {
            app.scroll_down();
        }
        KeyCode::PageUp => {
            app.scroll_up_page(5);
        }
        KeyCode::PageDown => {
            app.scroll_down_page(5);
        }
        KeyCode::Char(c) => {
            if !app.agent_active {
                app.input_char(c);
            }
        }
        _ => {}
    }
}

async fn agent_loop(
    config: &config::Config,
    initial_messages: &[app::Message],
    mode: Mode,
    _plan_artifact: Option<String>,
    event_tx: mpsc::Sender<AgentEvent>,
) {
    const MAX_ITERATIONS: usize = 10;

    let mut messages: Vec<app::Message> = initial_messages.to_vec();

    let tool_defs = tools::get_tool_definitions();

    for iteration in 0..MAX_ITERATIONS {
        debug!(
            "Agent loop iteration {}/{}, {} messages in history, mode={:?}",
            iteration + 1,
            MAX_ITERATIONS,
            messages.len(),
            mode
        );

        let max_attempts = config.retry_count + 1;
        let mut last_error = String::new();
        let mut completed = None;

        for attempt in 0..max_attempts {
            match api::stream_chat(
                config,
                &messages,
                Some(tool_defs.clone()),
                event_tx.clone(),
            )
            .await
            {
                Ok(tool_calls) => {
                    completed = Some(tool_calls);
                    break;
                }
                Err(e) => {
                    last_error = e;
                    if attempt + 1 < max_attempts {
                        let delay = config.retry_delay_secs * 2u32.pow(attempt);
                        let _ = event_tx.send(AgentEvent::Token(format!(
                            "\n[API call failed, retrying in {}s (attempt {}/{})...]\n",
                            delay,
                            attempt + 1,
                            max_attempts
                        )));
                        tokio::time::sleep(std::time::Duration::from_secs(delay.into())).await;
                    }
                }
            }
        }

        let completed = match completed {
            Some(c) => c,
            None => {
                let _ = event_tx.send(AgentEvent::Error(last_error));
                return;
            }
        };

        if completed.is_empty() {
            debug!("No tool calls, agent loop complete after {} iterations", iteration + 1);
            let _ = event_tx.send(AgentEvent::Done);
            return;
        }

        debug!("Got {} tool calls to execute", completed.len());

        // Add assistant message with tool calls to conversation history
        let assistant_content = String::new(); // content was already streamed as tokens
        messages.push(app::Message {
            role: app::MessageRole::Agent,
            content: assistant_content,
            tool_calls: Some(
                completed
                    .iter()
                    .map(|tc| app::ToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                    })
                    .collect(),
            ),
            tool_call_id: None,
            tool_result_error: false,
        });

        // Execute each tool call on a blocking thread to keep the runtime responsive
        for tc in &completed {
            debug!("Executing tool: {} with args: {}", tc.name, tc.arguments);
            let name = tc.name.clone();
            let arguments = tc.arguments.clone();
            let (result, is_error) = tokio::task::spawn_blocking(move || {
                tools::execute_tool(&name, &arguments)
            })
            .await
            .unwrap_or_else(|e| (format!("Tool execution panicked: {}", e), true));
            debug!(
                "Tool {} result (error={}): {}",
                tc.name,
                is_error,
                &result[..result.len().min(200)]
            );

            let _ = event_tx.send(AgentEvent::ToolCallEnd {
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments: tc.arguments.clone(),
                result: result.clone(),
                is_error,
            });

            // Add tool result to conversation history
            messages.push(app::Message {
                role: app::MessageRole::Tool,
                content: result,
                tool_calls: None,
                tool_call_id: Some(tc.id.clone()),
                tool_result_error: is_error,
            });
        }
    }

    // Max iterations reached
    debug!("Agent loop reached max iterations ({})", MAX_ITERATIONS);
    let _ = event_tx.send(AgentEvent::Error(format!(
        "Agent reached max iterations ({}) without completing the task.",
        MAX_ITERATIONS
    )));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_agent_loop_simple_text_response() {
        // This test verifies the agent loop completes when no tools are needed.
        // Since we can't easily mock the API, we test the channel flow.
        let (event_tx, event_rx) = mpsc::channel::<AgentEvent>();

        // Send Done manually to verify the App handles it
        event_tx.send(AgentEvent::Done).unwrap();

        let mut app = App::new();
        app.agent_active = true;
        app.streaming = true;

        while let Ok(event) = event_rx.try_recv() {
            app.handle_agent_event(event);
        }

        assert!(!app.agent_active);
        assert!(!app.streaming);
    }

    #[test]
    fn test_full_streaming_pipeline() {
        let mut app = App::new();

        app.input_char('H');
        app.input_char('i');
        let user_msg = app.send_message();
        assert!(user_msg.is_some(), "send_message should succeed with non-empty input");
        assert!(app.agent_active, "agent should be active after send");
        assert!(app.streaming, "app should be streaming after send");

        // Simulate the agent event flow
        let (event_tx, event_rx) = mpsc::channel::<AgentEvent>();

        event_tx.send(AgentEvent::Token("Hello".to_string())).unwrap();
        event_tx.send(AgentEvent::Token(" world".to_string())).unwrap();
        event_tx.send(AgentEvent::Token("!".to_string())).unwrap();
        event_tx.send(AgentEvent::Done).unwrap();

        while let Ok(event) = event_rx.try_recv() {
            app.handle_agent_event(event);
        }

        assert!(!app.agent_active, "agent should be done");
        assert!(!app.streaming, "streaming should be finished");
        let agent_msg = app.active_messages().last().unwrap();
        assert_eq!(agent_msg.role, app::MessageRole::Agent);
        assert_eq!(agent_msg.content, "Hello world!");
    }

    #[test]
    fn test_agent_loop_with_tool_calls() {
        let mut app = App::new();
        app.input_char('x'); // Need non-empty input for send_message to succeed
        app.send_message();

        let (event_tx, event_rx) = mpsc::channel::<AgentEvent>();

        // Simulate a token, then a tool call, then more tokens, then done
        event_tx.send(AgentEvent::Token("Let me".into())).unwrap();
        event_tx.send(AgentEvent::Token(" check".into())).unwrap();
        event_tx.send(AgentEvent::ToolCallStart {
            id: "call_1".into(),
            name: "ls".into(),
        }).unwrap();
        event_tx.send(AgentEvent::ToolCallArg {
            id: "call_1".into(),
            args: r#"{"path":"src"}"#.into(),
        }).unwrap();
        event_tx.send(AgentEvent::ToolCallEnd {
            id: "call_1".into(),
            name: "ls".into(),
            arguments: r#"{"path":"src"}"#.into(),
            result: "main.rs\napp.rs".into(),
            is_error: false,
        }).unwrap();
        event_tx.send(AgentEvent::Token("Found 2".into())).unwrap();
        event_tx.send(AgentEvent::Token(" files".into())).unwrap();
        event_tx.send(AgentEvent::Done).unwrap();

        while let Ok(event) = event_rx.try_recv() {
            app.handle_agent_event(event);
        }

        assert!(!app.agent_active);
        let msgs = app.active_messages();
        // welcome + prompt + user + agent("Let me check") + tool_call + tool_result + agent("Found 2 files")
        assert_eq!(msgs.len(), 7);
        assert_eq!(msgs[3].role, app::MessageRole::Agent);
        assert!(msgs[3].content.contains("Let me check"));
        assert_eq!(msgs[4].role, app::MessageRole::Agent);
        assert!(msgs[4].tool_calls.is_some());
        assert_eq!(msgs[5].role, app::MessageRole::Tool);
        assert_eq!(msgs[5].content, "main.rs\napp.rs");
        assert_eq!(msgs[6].role, app::MessageRole::Agent);
        assert_eq!(msgs[6].content, "Found 2 files");
    }
}
