use crate::config::Config;

pub struct App {
    pub messages: Vec<Message>,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll_offset: u16,
    pub should_quit: bool,
    pub streaming: bool,
    pub agent_active: bool,
    pub config: Config,
    pending_tool_calls: Vec<PendingToolCall>,
}

#[derive(Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub tool_result_error: bool,
}

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

struct PendingToolCall {
    id: String,
    arguments: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Agent,
    System,
    Tool,
}

pub enum AgentEvent {
    Token(String),
    ToolCallStart { id: String, name: String },
    ToolCallArg { id: String, args: String },
    ToolCallEnd { id: String, name: String, arguments: String, result: String, is_error: bool },
    Done,
    Error(String),
}

impl App {
    pub fn new() -> Self {
        Self {
            messages: vec![Message {
                role: MessageRole::System,
                content: "Welcome to Zero Code CLI. Type /help for available commands.".into(),
                tool_calls: None,
                tool_call_id: None,
                tool_result_error: false,
            }],
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            should_quit: false,
            streaming: false,
            agent_active: false,
            config: Config::load(),
            pending_tool_calls: Vec::new(),
        }
    }

    pub fn send_message(&mut self) -> Option<String> {
        let msg = self.input.trim().to_string();
        if msg.is_empty() || self.streaming || self.agent_active {
            return None;
        }
        self.messages.push(Message {
            role: MessageRole::User,
            content: msg.clone(),
            tool_calls: None,
            tool_call_id: None,
            tool_result_error: false,
        });
        self.input.clear();
        self.cursor_pos = 0;
        self.streaming = true;
        self.agent_active = true;
        Some(msg)
    }

    pub fn append_agent_token(&mut self, token: &str) {
        self.streaming = true;
        if let Some(last) = self.messages.last_mut()
            && last.role == MessageRole::Agent
        {
            last.content.push_str(token);
            return;
        }
        self.messages.push(Message {
            role: MessageRole::Agent,
            content: token.to_string(),
            tool_calls: None,
            tool_call_id: None,
            tool_result_error: false,
        });
    }

    pub fn add_system_message(&mut self, text: &str) {
        self.messages.push(Message {
            role: MessageRole::System,
            content: text.to_string(),
            tool_calls: None,
            tool_call_id: None,
            tool_result_error: false,
        });
    }

    pub fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Token(token) => {
                self.append_agent_token(&token);
            }
            AgentEvent::ToolCallStart { id, name: _name } => {
                self.pending_tool_calls.push(PendingToolCall {
                    id,
                    arguments: String::new(),
                });
            }
            AgentEvent::ToolCallArg { id, args } => {
                if let Some(tc) = self.pending_tool_calls.iter_mut().find(|tc| tc.id == id) {
                    tc.arguments.push_str(&args);
                }
            }
            AgentEvent::ToolCallEnd { id, name, arguments, result, is_error } => {
                self.pending_tool_calls.retain(|tc| tc.id != id);
                // Add tool-call message
                self.messages.push(Message {
                    role: MessageRole::Tool,
                    content: format!("{}({})", name, arguments),
                    tool_calls: Some(vec![ToolCall { id: id.clone(), name, arguments }]),
                    tool_call_id: None,
                    tool_result_error: false,
                });
                // Add tool-result message
                self.messages.push(Message {
                    role: MessageRole::Tool,
                    content: result,
                    tool_calls: None,
                    tool_call_id: Some(id),
                    tool_result_error: is_error,
                });
            }
            AgentEvent::Done => {
                self.agent_active = false;
                self.streaming = false;
            }
            AgentEvent::Error(err) => {
                self.add_system_message(&format!("Error: {}", err));
                self.agent_active = false;
                self.streaming = false;
            }
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_up_page(&mut self, page_size: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(page_size);
    }

    pub fn scroll_down_page(&mut self, page_size: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }

    pub fn input_char(&mut self, c: char) {
        let pos = self.cursor_pos.min(self.input.len());
        self.input.insert(pos, c);
        self.cursor_pos = pos + c.len_utf8();
    }

    pub fn delete_char(&mut self) {
        if self.cursor_pos > 0 {
            let mut remove_pos = self.cursor_pos - 1;
            while !self.input.is_char_boundary(remove_pos) {
                remove_pos -= 1;
            }
            self.input.remove(remove_pos);
            self.cursor_pos = remove_pos;
        }
    }

    pub fn delete_word(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let mut end = self.cursor_pos;
        while end > 0 && self.input.as_bytes().get(end - 1) == Some(&b' ') {
            end -= 1;
        }
        while end > 0 && self.input.as_bytes().get(end - 1) != Some(&b' ') {
            end -= 1;
        }
        self.input.drain(end..self.cursor_pos);
        self.cursor_pos = end;
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            let mut pos = self.cursor_pos - 1;
            while pos > 0 && !self.input.is_char_boundary(pos) {
                pos -= 1;
            }
            self.cursor_pos = pos;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            let mut pos = self.cursor_pos;
            loop {
                pos += 1;
                if pos >= self.input.len() || self.input.is_char_boundary(pos) {
                    break;
                }
            }
            self.cursor_pos = pos;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn input_mode(&self) -> &str {
        if self.agent_active {
            "AGENT"
        } else {
            "INSERT"
        }
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.input.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_first_token_creates_agent_message() {
        let mut app = App::new();
        assert_eq!(app.messages.len(), 1); // welcome system message

        app.append_agent_token("Hello");
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[1].role, MessageRole::Agent);
        assert_eq!(app.messages[1].content, "Hello");
    }

    #[test]
    fn test_append_token_appends_to_existing_agent() {
        let mut app = App::new();
        app.append_agent_token("Hello");
        app.append_agent_token(" world");

        assert_eq!(app.messages.len(), 2); // welcome + 1 agent
        assert_eq!(app.messages[1].content, "Hello world");
    }

    #[test]
    fn test_append_token_after_system_message_creates_new_agent() {
        let mut app = App::new();
        app.append_agent_token("Hi");
        app.add_system_message("Error: something");
        app.append_agent_token("Hello again");

        assert_eq!(app.messages.len(), 4);
        assert_eq!(app.messages[1].role, MessageRole::Agent);
        assert_eq!(app.messages[1].content, "Hi");
        assert_eq!(app.messages[2].role, MessageRole::System);
        assert_eq!(app.messages[3].role, MessageRole::Agent);
        assert_eq!(app.messages[3].content, "Hello again");
    }

    #[test]
    fn test_agent_event_token() {
        let mut app = App::new();
        app.handle_agent_event(AgentEvent::Token("Hi".into()));
        app.handle_agent_event(AgentEvent::Token(" there".into()));

        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[1].role, MessageRole::Agent);
        assert_eq!(app.messages[1].content, "Hi there");
        assert!(app.streaming);
    }

    #[test]
    fn test_agent_event_done() {
        let mut app = App::new();
        app.agent_active = true;
        app.streaming = true;
        app.handle_agent_event(AgentEvent::Done);

        assert!(!app.agent_active);
        assert!(!app.streaming);
    }

    #[test]
    fn test_agent_event_error() {
        let mut app = App::new();
        app.agent_active = true;
        app.streaming = true;
        app.handle_agent_event(AgentEvent::Error("boom".into()));

        assert!(!app.agent_active);
        assert!(!app.streaming);
        let last = app.messages.last().unwrap();
        assert_eq!(last.role, MessageRole::System);
        assert!(last.content.contains("boom"));
    }

    #[test]
    fn test_agent_event_tool_call_flow() {
        let mut app = App::new();

        // Agent begins text response
        app.handle_agent_event(AgentEvent::Token("Let me check".into()));

        // Model decides to call a tool
        app.handle_agent_event(AgentEvent::ToolCallStart {
            id: "call_1".into(),
            name: "ls".into(),
        });
        app.handle_agent_event(AgentEvent::ToolCallArg {
            id: "call_1".into(),
            args: r#"{"path":"#.into(),
        });
        app.handle_agent_event(AgentEvent::ToolCallArg {
            id: "call_1".into(),
            args: r#""src"}"#.into(),
        });

        // Tool execution completes
        app.handle_agent_event(AgentEvent::ToolCallEnd {
            id: "call_1".into(),
            name: "ls".into(),
            arguments: r#"{"path":"src"}"#.into(),
            result: "main.rs\napp.rs".into(),
            is_error: false,
        });

        // Agent continues after tool result
        app.handle_agent_event(AgentEvent::Token("Found 2 files".into()));
        app.handle_agent_event(AgentEvent::Done);

        // Verify message sequence: welcome, agent("Let me check"), tool_call, tool_result, agent("Found 2 files")
        assert_eq!(app.messages.len(), 5);
        assert_eq!(app.messages[0].role, MessageRole::System); // welcome
        assert_eq!(app.messages[1].role, MessageRole::Agent);
        assert_eq!(app.messages[1].content, "Let me check");
        assert_eq!(app.messages[2].role, MessageRole::Tool);
        assert!(app.messages[2].tool_calls.is_some());
        assert_eq!(app.messages[3].role, MessageRole::Tool);
        assert_eq!(app.messages[3].tool_call_id, Some("call_1".into()));
        assert_eq!(app.messages[3].content, "main.rs\napp.rs");
        assert_eq!(app.messages[4].role, MessageRole::Agent);
        assert_eq!(app.messages[4].content, "Found 2 files");
        assert!(!app.agent_active);
        assert!(!app.streaming);
    }

    #[test]
    fn test_send_message_blocked_when_agent_active() {
        let mut app = App::new();
        app.agent_active = true;
        app.input = "test".into();

        assert!(app.send_message().is_none());
        assert_eq!(app.messages.len(), 1); // only welcome message
    }

    #[test]
    fn test_send_message_blocked_when_streaming() {
        let mut app = App::new();
        app.streaming = true;
        app.input = "test".into();

        assert!(app.send_message().is_none());
    }

    #[test]
    fn test_input_mode_agent() {
        let mut app = App::new();
        assert_eq!(app.input_mode(), "INSERT");
        app.agent_active = true;
        assert_eq!(app.input_mode(), "AGENT");
    }
}
