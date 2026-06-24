use crate::config::Config;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Plan,
    Build,
}

pub struct App {
    pub current_mode: Mode,
    pub plan_messages: Vec<Message>,
    pub build_messages: Vec<Message>,
    pub plan_artifact: Option<String>,
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

const PLAN_SYSTEM_PROMPT: &str = "\
You are a software architect and technical design expert. Your role is to research, analyze, \
and create detailed software design documents.

When given a task:
1. First, explore and understand the problem domain thoroughly — use tools to examine the \
codebase, research approaches, and identify constraints.
2. Analyze requirements, trade-offs, and potential approaches.
3. Produce a comprehensive software design document covering architecture, components, \
data flow, interfaces, key decisions, and risks.

IMPORTANT: Do NOT write implementation code. Focus exclusively on design and planning. \
Your output will be used by a separate build agent to implement the actual code.";

const BUILD_SYSTEM_PROMPT: &str = "\
You are a coding assistant. Write production-quality, safe, and efficient code. \
Follow best practices and produce working implementations.";

impl App {
    pub fn new() -> Self {
        let welcome = Message {
            role: MessageRole::System,
            content: "Welcome to Zero Code CLI. Type /help for available commands.".into(),
            tool_calls: None,
            tool_call_id: None,
            tool_result_error: false,
        };
        Self {
            current_mode: Mode::Plan,
            plan_messages: vec![
                welcome.clone(),
                Message {
                    role: MessageRole::System,
                    content: PLAN_SYSTEM_PROMPT.into(),
                    tool_calls: None,
                    tool_call_id: None,
                    tool_result_error: false,
                },
            ],
            build_messages: vec![
                welcome,
                Message {
                    role: MessageRole::System,
                    content: BUILD_SYSTEM_PROMPT.into(),
                    tool_calls: None,
                    tool_call_id: None,
                    tool_result_error: false,
                },
            ],
            plan_artifact: None,
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

    pub fn active_messages(&self) -> &Vec<Message> {
        match self.current_mode {
            Mode::Plan => &self.plan_messages,
            Mode::Build => &self.build_messages,
        }
    }

    pub fn active_messages_mut(&mut self) -> &mut Vec<Message> {
        match self.current_mode {
            Mode::Plan => &mut self.plan_messages,
            Mode::Build => &mut self.build_messages,
        }
    }

    pub fn switch_mode(&mut self, mode: Mode) {
        if self.current_mode == mode {
            return;
        }
        // Capture plan artifact when switching from plan to build
        if self.current_mode == Mode::Plan && mode == Mode::Build {
            self.capture_plan_artifact();
        }
        self.current_mode = mode;
        self.input.clear();
        self.cursor_pos = 0;
        self.scroll_offset = 0;
        self.pending_tool_calls.clear();
    }

    fn capture_plan_artifact(&mut self) {
        let agent_texts: Vec<&str> = self
            .plan_messages
            .iter()
            .filter(|m| m.role == MessageRole::Agent && !m.content.is_empty())
            .map(|m| m.content.as_str())
            .collect();
        if agent_texts.is_empty() {
            return;
        }
        self.plan_artifact = Some(agent_texts.join("\n\n"));
    }

    pub fn send_message(&mut self) -> Option<String> {
        let msg = self.input.trim().to_string();
        if msg.is_empty() || self.streaming || self.agent_active {
            return None;
        }

        // If build mode and plan_artifact exists, inject it before the user message
        if self.current_mode == Mode::Build
            && let Some(ref artifact) = self.plan_artifact
            && !self.build_messages.iter().any(|m| m.content.contains(artifact.as_str()))
        {
            self.build_messages.push(Message {
                role: MessageRole::System,
                content: format!(
                    "You have been given a software design plan. Follow it closely when implementing.\n\
                     If you find issues with the plan, note them but still follow the overall architecture.\n\
                     \nHere is the design plan:\n---\n{}\n---",
                    artifact
                ),
                tool_calls: None,
                tool_call_id: None,
                tool_result_error: false,
            });
        }

        self.active_messages_mut().push(Message {
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

    pub fn handle_slash_command(&mut self, cmd: &str) -> bool {
        match cmd {
            "/new" => {
                self.reset_session();
                true
            }
            "/plan" => {
                self.switch_mode(Mode::Plan);
                true
            }
            "/build" => {
                self.switch_mode(Mode::Build);
                true
            }
            _ => false,
        }
    }

    fn reset_session(&mut self) {
        let welcome = Message {
            role: MessageRole::System,
            content: "Welcome to Zero Code CLI. Type /help for available commands.".into(),
            tool_calls: None,
            tool_call_id: None,
            tool_result_error: false,
        };
        self.plan_messages = vec![
            welcome.clone(),
            Message {
                role: MessageRole::System,
                content: PLAN_SYSTEM_PROMPT.into(),
                tool_calls: None,
                tool_call_id: None,
                tool_result_error: false,
            },
        ];
        self.build_messages = vec![
            welcome,
            Message {
                role: MessageRole::System,
                content: BUILD_SYSTEM_PROMPT.into(),
                tool_calls: None,
                tool_call_id: None,
                tool_result_error: false,
            },
        ];
        self.plan_artifact = None;
        self.input.clear();
        self.cursor_pos = 0;
        self.scroll_offset = 0;
        self.pending_tool_calls.clear();
        self.streaming = false;
        self.agent_active = false;
    }

    pub fn append_agent_token(&mut self, token: &str) {
        self.streaming = true;
        let messages = self.active_messages_mut();
        if let Some(last) = messages.last_mut()
            && last.role == MessageRole::Agent
        {
            last.content.push_str(token);
            return;
        }
        messages.push(Message {
            role: MessageRole::Agent,
            content: token.to_string(),
            tool_calls: None,
            tool_call_id: None,
            tool_result_error: false,
        });
    }

    pub fn add_system_message(&mut self, text: &str) {
        self.active_messages_mut().push(Message {
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
                let messages = self.active_messages_mut();
                messages.push(Message {
                    role: MessageRole::Agent,
                    content: format!("{}({})", name, arguments),
                    tool_calls: Some(vec![ToolCall { id: id.clone(), name, arguments }]),
                    tool_call_id: None,
                    tool_result_error: false,
                });
                messages.push(Message {
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

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.input.len();
    }

    pub fn input_mode(&self) -> &str {
        if self.agent_active {
            "AGENT"
        } else {
            match self.current_mode {
                Mode::Plan => "PLAN",
                Mode::Build => "BUILD",
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let app = App::new();
        assert_eq!(app.current_mode, Mode::Plan);
        // Each mode has welcome + mode system prompt = 2 messages
        assert_eq!(app.plan_messages.len(), 2);
        assert_eq!(app.build_messages.len(), 2);
        assert_eq!(app.plan_messages[0].content, "Welcome to Zero Code CLI. Type /help for available commands.");
        assert_eq!(app.plan_messages[1].content, PLAN_SYSTEM_PROMPT);
        assert_eq!(app.build_messages[1].content, BUILD_SYSTEM_PROMPT);
        assert!(app.plan_artifact.is_none());
        assert!(!app.agent_active);
        assert!(!app.streaming);
        assert_eq!(app.input_mode(), "PLAN");
    }

    #[test]
    fn test_active_messages_respects_mode() {
        let mut app = App::new();
        // Plan mode by default — push a user message
        app.input = "plan msg".into();
        app.send_message();
        assert_eq!(app.plan_messages.len(), 3); // welcome + prompt + user
        assert_eq!(app.build_messages.len(), 2); // unchanged
    }

    #[test]
    fn test_append_first_token_creates_agent_message() {
        let mut app = App::new();
        let initial_len = app.active_messages().len();
        app.append_agent_token("Hello");
        assert_eq!(app.active_messages().len(), initial_len + 1);
        let last = app.active_messages().last().unwrap();
        assert_eq!(last.role, MessageRole::Agent);
        assert_eq!(last.content, "Hello");
    }

    #[test]
    fn test_append_token_appends_to_existing_agent() {
        let mut app = App::new();
        app.append_agent_token("Hello");
        app.append_agent_token(" world");
        let last = app.active_messages().last().unwrap();
        assert_eq!(last.content, "Hello world");
    }

    #[test]
    fn test_append_token_after_system_message_creates_new_agent() {
        let mut app = App::new();
        app.append_agent_token("Hi");
        app.add_system_message("Error: something");
        app.append_agent_token("Hello again");
        let msgs = app.active_messages();
        let len = msgs.len();
        assert_eq!(msgs[len - 3].role, MessageRole::Agent);
        assert_eq!(msgs[len - 3].content, "Hi");
        assert_eq!(msgs[len - 2].role, MessageRole::System);
        assert_eq!(msgs[len - 1].role, MessageRole::Agent);
        assert_eq!(msgs[len - 1].content, "Hello again");
    }

    #[test]
    fn test_agent_event_token() {
        let mut app = App::new();
        let initial_len = app.active_messages().len();
        app.handle_agent_event(AgentEvent::Token("Hi".into()));
        app.handle_agent_event(AgentEvent::Token(" there".into()));
        assert_eq!(app.active_messages().len(), initial_len + 1);
        let last = app.active_messages().last().unwrap();
        assert_eq!(last.role, MessageRole::Agent);
        assert_eq!(last.content, "Hi there");
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
        let last = app.active_messages().last().unwrap();
        assert_eq!(last.role, MessageRole::System);
        assert!(last.content.contains("boom"));
    }

    #[test]
    fn test_agent_event_tool_call_flow() {
        let mut app = App::new();
        let initial_len = app.active_messages().len();

        app.handle_agent_event(AgentEvent::Token("Let me check".into()));
        app.handle_agent_event(AgentEvent::ToolCallStart { id: "call_1".into(), name: "ls".into() });
        app.handle_agent_event(AgentEvent::ToolCallArg { id: "call_1".into(), args: r#"{"path":"#.into() });
        app.handle_agent_event(AgentEvent::ToolCallArg { id: "call_1".into(), args: r#""src"}"#.into() });
        app.handle_agent_event(AgentEvent::ToolCallEnd {
            id: "call_1".into(),
            name: "ls".into(),
            arguments: r#"{"path":"src"}"#.into(),
            result: "main.rs\napp.rs".into(),
            is_error: false,
        });
        app.handle_agent_event(AgentEvent::Token("Found 2 files".into()));
        app.handle_agent_event(AgentEvent::Done);

        let msgs = app.active_messages();
        // initial (2 welcome/prompt) + agent("Let me check") + tool_call + tool_result + agent("Found 2 files")
        assert_eq!(msgs.len(), initial_len + 4);
        assert_eq!(msgs[initial_len].role, MessageRole::Agent);
        assert_eq!(msgs[initial_len].content, "Let me check");
        assert_eq!(msgs[initial_len + 1].role, MessageRole::Agent);
        assert!(msgs[initial_len + 1].tool_calls.is_some());
        assert_eq!(msgs[initial_len + 2].role, MessageRole::Tool);
        assert_eq!(msgs[initial_len + 2].tool_call_id, Some("call_1".into()));
        assert_eq!(msgs[initial_len + 2].content, "main.rs\napp.rs");
        assert_eq!(msgs[initial_len + 3].role, MessageRole::Agent);
        assert_eq!(msgs[initial_len + 3].content, "Found 2 files");
        assert!(!app.agent_active);
        assert!(!app.streaming);
    }

    #[test]
    fn test_send_message_blocked_when_agent_active() {
        let mut app = App::new();
        app.agent_active = true;
        app.input = "test".into();
        assert!(app.send_message().is_none());
        // Only welcome + mode prompt
        assert_eq!(app.active_messages().len(), 2);
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
        app.agent_active = true;
        assert_eq!(app.input_mode(), "AGENT");
    }

    #[test]
    fn test_input_mode_plan() {
        let app = App::new();
        assert_eq!(app.input_mode(), "PLAN");
    }

    #[test]
    fn test_input_mode_build() {
        let mut app = App::new();
        app.switch_mode(Mode::Build);
        assert_eq!(app.input_mode(), "BUILD");
    }

    #[test]
    fn test_slash_command_new_resets_both_modes() {
        let mut app = App::new();
        // Add messages to both modes
        app.plan_messages.push(Message {
            role: MessageRole::User, content: "p".into(),
            tool_calls: None, tool_call_id: None, tool_result_error: false,
        });
        app.build_messages.push(Message {
            role: MessageRole::User, content: "b".into(),
            tool_calls: None, tool_call_id: None, tool_result_error: false,
        });
        app.plan_artifact = Some("plan".into());
        app.input = "some text".into();
        app.cursor_pos = 5;
        app.scroll_offset = 10;
        app.streaming = true;
        app.agent_active = true;

        let handled = app.handle_slash_command("/new");
        assert!(handled);

        // Both modes reset to welcome + mode prompt
        assert_eq!(app.plan_messages.len(), 2);
        assert_eq!(app.plan_messages[0].role, MessageRole::System);
        assert_eq!(app.build_messages.len(), 2);
        assert!(app.plan_artifact.is_none());
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);
        assert!(!app.streaming);
        assert!(!app.agent_active);
    }

    #[test]
    fn test_slash_command_unknown_returns_false() {
        let mut app = App::new();
        let handled = app.handle_slash_command("/foo");
        assert!(!handled);
    }

    #[test]
    fn test_switch_mode_plan_to_build_captures_artifact() {
        let mut app = App::new();
        // Add agent messages to plan mode
        app.plan_messages.push(Message {
            role: MessageRole::Agent,
            content: "Design: use async Rust".into(),
            tool_calls: None, tool_call_id: None, tool_result_error: false,
        });
        app.plan_messages.push(Message {
            role: MessageRole::Agent,
            content: "Architecture: event-driven".into(),
            tool_calls: None, tool_call_id: None, tool_result_error: false,
        });

        app.switch_mode(Mode::Build);
        assert_eq!(app.current_mode, Mode::Build);
        assert!(app.plan_artifact.is_some());
        let artifact = app.plan_artifact.unwrap();
        assert!(artifact.contains("Design: use async Rust"));
        assert!(artifact.contains("Architecture: event-driven"));
        assert!(app.input.is_empty());
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_switch_mode_build_to_plan_no_capture() {
        let mut app = App::new();
        app.switch_mode(Mode::Build);
        app.plan_artifact = None;

        app.switch_mode(Mode::Plan);
        assert_eq!(app.current_mode, Mode::Plan);
        assert!(app.plan_artifact.is_none());
    }

    #[test]
    fn test_switch_mode_same_mode_noop() {
        let mut app = App::new();
        app.input = "hello".into();
        app.cursor_pos = 3;
        app.switch_mode(Mode::Plan); // same as default
        assert_eq!(app.current_mode, Mode::Plan);
        assert_eq!(app.input, "hello"); // unchanged — no reset
    }

    #[test]
    fn test_slash_plan() {
        let mut app = App::new();
        app.switch_mode(Mode::Build);
        assert!(app.handle_slash_command("/plan"));
        assert_eq!(app.current_mode, Mode::Plan);
    }

    #[test]
    fn test_slash_build() {
        let mut app = App::new();
        assert!(app.handle_slash_command("/build"));
        assert_eq!(app.current_mode, Mode::Build);
    }

    #[test]
    fn test_plan_artifact_injected_on_build_send() {
        let mut app = App::new();
        // Set up: switch to build mode with a plan artifact
        app.plan_messages.push(Message {
            role: MessageRole::Agent,
            content: "Use hexagonal architecture".into(),
            tool_calls: None, tool_call_id: None, tool_result_error: false,
        });
        app.switch_mode(Mode::Build);

        // Send a message in build mode
        app.input = "implement it".into();
        app.send_message();

        // Check that the artifact was injected as a system message
        let build_msgs = &app.build_messages;
        // welcome + build prompt + plan artifact system msg + user "implement it"
        assert_eq!(build_msgs.len(), 4);
        assert_eq!(build_msgs[2].role, MessageRole::System);
        assert!(build_msgs[2].content.contains("Use hexagonal architecture"));
        assert!(build_msgs[2].content.contains("Here is the design plan"));
        assert_eq!(build_msgs[3].role, MessageRole::User);
        assert_eq!(build_msgs[3].content, "implement it");
    }

    #[test]
    fn test_plan_artifact_not_injected_twice() {
        let mut app = App::new();
        app.plan_messages.push(Message {
            role: MessageRole::Agent,
            content: "Use hexagonal architecture".into(),
            tool_calls: None, tool_call_id: None, tool_result_error: false,
        });
        app.switch_mode(Mode::Build);

        // First message injects the artifact
        app.input = "first msg".into();
        app.send_message();
        // Manually reset for second message
        app.agent_active = false;
        app.streaming = false;
        app.input = "second msg".into();
        app.send_message();

        let build_msgs = &app.build_messages;
        // welcome + build prompt + artifact system + user "first" + user "second"
        // Only one artifact injection
        let artifact_count = build_msgs
            .iter()
            .filter(|m| m.content.contains("Here is the design plan"))
            .count();
        assert_eq!(artifact_count, 1);
    }
}
