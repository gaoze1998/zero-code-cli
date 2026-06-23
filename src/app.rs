use crate::config::Config;

pub struct App {
    pub messages: Vec<Message>,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll_offset: u16,
    pub should_quit: bool,
    pub streaming: bool,
    pub config: Config,
}

pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Agent,
    System,
}

impl App {
    pub fn new() -> Self {
        Self {
            messages: vec![Message {
                role: MessageRole::System,
                content: "Welcome to Zero Code CLI. Type /help for available commands.".into(),
            }],
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            should_quit: false,
            streaming: false,
            config: Config::load(),
        }
    }

    pub fn send_message(&mut self) -> Option<String> {
        let msg = self.input.trim().to_string();
        if msg.is_empty() || self.streaming {
            return None;
        }
        self.messages.push(Message {
            role: MessageRole::User,
            content: msg.clone(),
        });
        self.input.clear();
        self.cursor_pos = 0;
        self.streaming = true;
        Some(msg)
    }

    pub fn append_agent_token(&mut self, token: &str) {
        if let Some(last) = self.messages.last_mut()
            && last.role == MessageRole::Agent
        {
            last.content.push_str(token);
            return;
        }
        self.messages.push(Message {
            role: MessageRole::Agent,
            content: token.to_string(),
        });
    }

    pub fn finish_streaming(&mut self) {
        self.streaming = false;
    }

    pub fn add_system_message(&mut self, text: &str) {
        self.messages.push(Message {
            role: MessageRole::System,
            content: text.to_string(),
        });
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
        "INSERT"
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.input.len();
    }
}
