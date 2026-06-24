use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, MessageRole, Mode};
#[cfg(test)]
use crate::app::Message;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // tab bar
            Constraint::Min(3),     // conversation
            Constraint::Length(3),  // input
            Constraint::Length(1),  // status bar
        ])
        .split(f.area());

    draw_tab_bar(f, chunks[0], app);
    draw_conversation(f, chunks[1], app);
    draw_input(f, chunks[2], app);
    draw_status_bar(f, chunks[3], app);
}

fn draw_tab_bar(f: &mut Frame, area: Rect, app: &App) {
    let plan_style = if app.current_mode == Mode::Plan {
        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let build_style = if app.current_mode == Mode::Build {
        Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let tabs = Line::from(vec![
        Span::styled(" Plan ", plan_style),
        Span::raw(" "),
        Span::styled(" Build ", build_style),
    ]);

    let paragraph = Paragraph::new(Text::from(vec![tabs]))
        .block(Block::default());
    f.render_widget(paragraph, area);
}

fn draw_conversation(f: &mut Frame, area: Rect, app: &App) {
    let mut messages: Vec<Line> = Vec::new();

    // In Build mode, show plan's agent output as context above the build conversation
    if app.current_mode == Mode::Build {
        for msg in app.plan_messages.iter().skip(2) {
            if msg.role == MessageRole::Agent && !msg.content.is_empty() {
                messages.push(Line::from(Span::styled(
                    format!("[Plan] {}", msg.content),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
        if !messages.is_empty() {
            messages.push(Line::from(Span::styled(
                "─── Build ───",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
        }
    }

    for msg in app.active_messages().iter().skip(2) {
        // Skip the initial welcome + mode prompt for cleaner display
        match msg.role {
            MessageRole::Tool => {
                if msg.tool_calls.is_some() {
                    if let Some(ref tcs) = msg.tool_calls
                        && let Some(tc) = tcs.first()
                    {
                        messages.push(Line::from(vec![
                            Span::styled(
                                format!("{}(", tc.name),
                                Style::default()
                                    .fg(Color::Magenta)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                &tc.arguments,
                                Style::default().fg(Color::Magenta),
                            ),
                            Span::styled(
                                ")",
                                Style::default()
                                    .fg(Color::Magenta)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    }
                } else {
                    let style = if msg.tool_result_error {
                        Style::default().fg(Color::Red)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    let display = if msg.content.len() > 500 {
                        format!("{}... (truncated)", &msg.content[..500])
                    } else {
                        msg.content.clone()
                    };
                    messages.push(Line::from(Span::styled(
                        format!("  {}", display),
                        style,
                    )));
                }
            }
            _ => {
                let (role_label, color) = match msg.role {
                    MessageRole::User => ("You", Color::Green),
                    MessageRole::Agent => ("Agent", Color::Cyan),
                    MessageRole::System => ("System", Color::Yellow),
                    MessageRole::Tool => unreachable!(),
                };
                messages.push(
                    vec![
                        Span::styled(
                            format!("{}: ", role_label),
                            Style::default()
                                .fg(color)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(&msg.content),
                    ]
                    .into(),
                );
            }
        }
    }

    // Show streaming indicator on last agent message
    if app.streaming
        && let Some(last) = app.active_messages().last()
        && last.role == MessageRole::Agent
    {
        messages.push(Line::from(Span::styled(
            "▌",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::RAPID_BLINK),
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
    let mode_label = match app.current_mode {
        Mode::Plan => "Plan",
        Mode::Build => "Build",
    };
    let block = if app.agent_active {
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Input [{}] (agent...) ", mode_label))
    } else {
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Input [{}] ", mode_label))
    };

    let prompt = Span::styled(
        "> ",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    );

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
        Line::from(vec![
            prompt,
            Span::raw(before),
            highlight,
            Span::raw(after_str),
        ])
    };

    let paragraph = Paragraph::new(Text::from(vec![line])).block(block);
    f.render_widget(paragraph, area);
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mode_label = app.input_mode();
    let mode_text = if mode_label == "AGENT" {
        Span::styled(
            " AGENT ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        let color = match app.current_mode {
            Mode::Plan => Color::Cyan,
            Mode::Build => Color::Green,
        };
        Span::styled(
            format!(" {} ", mode_label),
            Style::default()
                .fg(Color::Black)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        )
    };

    let status = Line::from(vec![
        mode_text,
        Span::raw(" │ "),
        Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" switch │ "),
        Span::styled(
            "Enter",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" send │ "),
        Span::styled(
            "Ctrl+C",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" quit │ "),
        Span::styled(
            "Ctrl+W",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" delete word │ "),
        Span::styled(
            "/new",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" clear │ "),
        Span::styled(
            "↑↓",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" scroll"),
    ]);

    let paragraph = Paragraph::new(status)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().style(Style::default().bg(Color::Rgb(
            30, 30, 30,
        ))));

    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

    fn buffer_contains(buffer: &Buffer, text: &str) -> bool {
        let area = buffer.area();
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
        app.active_messages_mut().push(Message {
            role: MessageRole::Agent,
            content: "Thinking...".into(),
            tool_calls: None,
            tool_call_id: None,
            tool_result_error: false,
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
    fn test_agent_in_status_bar() {
        let mut app = App::new();
        app.agent_active = true;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "AGENT"),
            "status bar should show AGENT during agent activity"
        );
    }

    #[test]
    fn test_plan_in_status_bar() {
        let mut app = App::new();
        app.agent_active = false;
        app.streaming = false;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "PLAN"),
            "status bar should show PLAN when idle in plan mode"
        );
    }

    #[test]
    fn test_build_in_status_bar() {
        let mut app = App::new();
        app.switch_mode(Mode::Build);
        app.agent_active = false;
        app.streaming = false;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "BUILD"),
            "status bar should show BUILD when idle in build mode"
        );
    }

    #[test]
    fn test_input_agent_title() {
        let mut app = App::new();
        app.agent_active = true;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "agent..."),
            "input panel should show 'agent...' during agent activity"
        );
    }

    #[test]
    fn test_input_normal_title_shows_mode() {
        let mut app = App::new();
        app.agent_active = false;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "Input [Plan]"),
            "input panel should show mode in title when idle"
        );
    }

    #[test]
    fn test_tab_bar_plan_active() {
        let app = App::new();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "Plan"),
            "tab bar should show Plan"
        );
        assert!(
            buffer_contains(buffer, "Build"),
            "tab bar should show Build"
        );
    }

    #[test]
    fn test_tool_call_renders() {
        let mut app = App::new();
        app.active_messages_mut().push(Message {
            role: MessageRole::Tool,
            content: "ls({\"path\":\"src\"})".into(),
            tool_calls: Some(vec![crate::app::ToolCall {
                id: "call_1".into(),
                name: "ls".into(),
                arguments: "{\"path\":\"src\"}".into(),
            }]),
            tool_call_id: None,
            tool_result_error: false,
        });

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "ls("),
            "tool call should show function name"
        );
    }

    #[test]
    fn test_tool_result_renders() {
        let mut app = App::new();
        app.active_messages_mut().push(Message {
            role: MessageRole::Tool,
            content: "main.rs\napi.rs".into(),
            tool_calls: None,
            tool_call_id: Some("call_1".into()),
            tool_result_error: false,
        });

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert!(
            buffer_contains(buffer, "main.rs"),
            "tool result should show content"
        );
    }
}
