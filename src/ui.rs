use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Message, MessageRole};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_conversation(f, chunks[0], app);
    draw_input(f, chunks[1], app);
    draw_status_bar(f, chunks[2], app);
}

fn draw_conversation(f: &mut Frame, area: Rect, app: &App) {
    let mut messages: Vec<Line> = app
        .messages
        .iter()
        .map(|msg| {
            let (role_label, color) = match msg.role {
                MessageRole::User => ("You", Color::Green),
                MessageRole::Agent => ("Agent", Color::Cyan),
                MessageRole::System => ("System", Color::Yellow),
            };
            vec![
                Span::styled(
                    format!("{}: ", role_label),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&msg.content),
            ]
            .into()
        })
        .collect();

    // Show streaming indicator on last agent message
    if app.streaming
        && let Some(last) = app.messages.last()
        && last.role == MessageRole::Agent
    {
        messages.push(Line::from(Span::styled(
            "▌",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::RAPID_BLINK),
        )));
    }

    let text = Text::from(messages);
    let paragraph = Paragraph::new(text)
        .block(Block::default().title(" Conversation ").borders(Borders::ALL))
        .wrap(Wrap { trim: true })
        .scroll((app.scroll_offset, 0));

    f.render_widget(paragraph, area);
}

fn draw_input(f: &mut Frame, area: Rect, app: &App) {
    let block = if app.streaming {
        Block::default()
            .borders(Borders::ALL)
            .title(" Input (waiting...) ")
    } else {
        Block::default().borders(Borders::ALL).title(" Input ")
    };

    let prompt = Span::styled("> ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));

    let line = if app.input.is_empty() {
        let cursor = Span::styled(" ", Style::default().bg(Color::White));
        Line::from(vec![prompt, cursor])
    } else {
        let before = &app.input[..app.cursor_pos.min(app.input.len())];
        let after_str: String;
        let highlight;
        if app.cursor_pos < app.input.len() {
            let rest = &app.input[app.cursor_pos..];
            let mut chars = rest.chars();
            let cur = chars.next().unwrap();
            after_str = chars.collect();
            highlight = Span::styled(
                cur.to_string(),
                Style::default().fg(Color::Black).bg(Color::White),
            );
        } else {
            highlight = Span::styled(
                " ",
                Style::default().fg(Color::Black).bg(Color::White),
            );
            after_str = String::new();
        }
        Line::from(vec![prompt, Span::raw(before), highlight, Span::raw(after_str)])
    };

    let paragraph = Paragraph::new(Text::from(vec![line])).block(block);
    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

    /// Collect all cell symbols from the buffer into a single String,
    /// then check if the target text appears anywhere.
    fn buffer_contains(buffer: &Buffer, text: &str) -> bool {
        let area = buffer.area();
        // Build per-row strings and join with newlines to preserve layout
        let mut content = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                content.push_str(buffer.cell((x, y)).unwrap().symbol());
            }
            content.push('\n');
        }
        content.contains(text)
    }

    #[test]
    fn test_streaming_cursor_renders() {
        let mut app = App::new();
        app.streaming = true;
        app.messages.push(Message {
            role: MessageRole::Agent,
            content: "Thinking...".into(),
        });

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "▌"),
            "streaming cursor ▌ should be present"
        );
    }

    #[test]
    fn test_no_streaming_cursor_when_idle() {
        let mut app = App::new();
        app.streaming = false;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            !buffer_contains(buffer, "▌"),
            "no streaming cursor should appear when idle"
        );
    }

    #[test]
    fn test_await_in_status_bar() {
        let mut app = App::new();
        app.streaming = true;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "AWAIT"),
            "status bar should show AWAIT during streaming"
        );
    }

    #[test]
    fn test_insert_in_status_bar() {
        let mut app = App::new();
        app.streaming = false;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "INSERT"),
            "status bar should show INSERT when idle"
        );
    }

    #[test]
    fn test_input_waiting_title() {
        let mut app = App::new();
        app.streaming = true;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "waiting..."),
            "input panel should show 'waiting...' during streaming"
        );
    }

    #[test]
    fn test_input_normal_title() {
        let mut app = App::new();
        app.streaming = false;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        // " Input " (with spaces) is the normal title
        assert!(
            buffer_contains(buffer, " Input "),
            "input panel should show normal ' Input ' title when idle"
        );
    }
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mode_text = if app.streaming {
        Span::styled(
            " AWAIT ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        let mode = app.input_mode();
        Span::styled(
            format!(" {} ", mode),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    };

    let status = Line::from(vec![
        mode_text,
        Span::raw(" │ "),
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" send │ "),
        Span::styled("Ctrl+C", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" quit │ "),
        Span::styled("Ctrl+W", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" delete word │ "),
        Span::styled("↑↓", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" scroll"),
    ]);

    let paragraph = Paragraph::new(status)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().style(Style::default().bg(Color::Rgb(30, 30, 30))));

    f.render_widget(paragraph, area);
}
