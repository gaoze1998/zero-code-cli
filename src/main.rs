#![forbid(unsafe_code)]

mod api;
mod app;
mod config;
mod ui;

use std::io::{self, stdout};
use std::sync::mpsc;
use std::time::Duration;

use app::App;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

fn main() -> io::Result<()> {
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

    let (tx, rx) = mpsc::channel::<String>();
    let (err_tx, err_rx) = mpsc::channel::<String>();
    let (done_tx, done_rx) = mpsc::channel::<()>();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // Drive the tokio runtime so spawned async tasks make progress
        rt.block_on(tokio::task::yield_now());

        // Drain streaming tokens
        while let Ok(token) = rx.try_recv() {
            eprintln!("[DEBUG] Token received: {:?}", token);
            app.append_agent_token(&token);
        }
        // Drain error messages
        while let Ok(err) = err_rx.try_recv() {
            eprintln!("[DEBUG] Error received: {}", err);
            app.add_system_message(&format!("Error: {}", err));
            app.finish_streaming();
        }
        // Check for stream completion
        if done_rx.try_recv().is_ok() {
            eprintln!("[DEBUG] Stream done signal received");
            app.finish_streaming();
        }

        // Poll for input
        if event::poll(Duration::from_millis(16)).unwrap_or(false) {
            match event::read()? {
                Event::Key(key) => {
                    handle_key(&mut app, key, &rt, &tx, &err_tx, &done_tx);
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
    tx: &mpsc::Sender<String>,
    err_tx: &mpsc::Sender<String>,
    done_tx: &mpsc::Sender<()>,
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
        KeyCode::Enter => {
            if let Some(_msg) = app.send_message() {
                let config = app.config.clone();
                let conversation: Vec<app::Message> = app
                    .messages
                    .iter()
                    .map(|m| app::Message {
                        role: m.role,
                        content: m.content.clone(),
                    })
                    .collect();
                let tx = tx.clone();
                let err_tx = err_tx.clone();
                let done_tx = done_tx.clone();
                rt.spawn(async move {
                    match api::stream_chat(&config, &conversation, tx).await {
                        Ok(()) => {
                            let _ = done_tx.send(());
                        }
                        Err(e) => {
                            let _ = err_tx.send(e);
                        }
                    }
                });
            }
        }
        KeyCode::Backspace => {
            if !app.streaming {
                app.delete_char();
            }
        }
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if !app.streaming {
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
            if !app.streaming {
                app.input_char(c);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_full_streaming_pipeline() {
        let mut app = app::App::new();

        // Simulate user typing and sending a message
        app.input_char('H');
        app.input_char('i');
        let user_msg = app.send_message();
        assert!(user_msg.is_some(), "send_message should succeed with non-empty input");
        assert!(app.streaming, "app should be streaming after send");

        // Simulate the API → channel → drain flow (mini event loop)
        let (tx, rx) = mpsc::channel::<String>();
        let (done_tx, done_rx) = mpsc::channel::<()>();

        tx.send("Hello".to_string()).unwrap();
        tx.send(" world".to_string()).unwrap();
        tx.send("!".to_string()).unwrap();
        done_tx.send(()).unwrap();

        // Drain tokens (mirrors run() logic)
        while let Ok(token) = rx.try_recv() {
            app.append_agent_token(&token);
        }
        if done_rx.try_recv().is_ok() {
            app.finish_streaming();
        }

        // Assert final state
        assert!(!app.streaming, "streaming should be finished");
        let agent_msg = app.messages.last().unwrap();
        assert_eq!(agent_msg.role, app::MessageRole::Agent);
        assert_eq!(agent_msg.content, "Hello world!");
    }
}
