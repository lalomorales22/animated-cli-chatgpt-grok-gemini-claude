use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::ai::{AIProvider, AIClient, Message};
use crate::db::Database;

#[derive(Debug, Clone)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[allow(dead_code)]
    pub timestamp: Instant,
    pub is_system: bool,
}

pub struct ChatInterface {
    provider: AIProvider,
    ai_client: AIClient,
    // Store messages per provider
    messages_per_provider: HashMap<String, Vec<ChatMessage>>,
    input_buffer: String,
    scroll_offset: usize,
    is_streaming: bool,
    show_help: bool,
    response_rx: mpsc::UnboundedReceiver<Result<String>>,
    response_tx: mpsc::UnboundedSender<Result<String>>,
    db: Option<Database>,
}

impl ChatInterface {
    pub fn new(provider: AIProvider) -> Self {
        let ai_client = AIClient::new(provider.clone());
        let (response_tx, response_rx) = mpsc::unbounded_channel();

        // Initialize database
        let db = match Database::new() {
            Ok(db) => Some(db),
            Err(_e) => None,
        };

        let mut chat = Self {
            provider: provider.clone(),
            ai_client,
            messages_per_provider: HashMap::new(),
            input_buffer: String::new(),
            scroll_offset: 0,
            is_streaming: false,
            show_help: false,
            response_rx,
            response_tx,
            db,
        };

        // Load history from database for all providers
        chat.load_all_histories();

        chat
    }

    fn load_all_histories(&mut self) {
        if let Some(ref db) = self.db {
            let providers = [
                AIProvider::Claude,
                AIProvider::Grok,
                AIProvider::OpenAI,
                AIProvider::Gemini,
            ];

            for provider in &providers {
                if let Ok(db_messages) = db.get_messages(provider.db_name()) {
                    let mut messages = Vec::new();
                    for db_msg in db_messages {
                        let role = match db_msg.role.as_str() {
                            "user" => MessageRole::User,
                            _ => MessageRole::Assistant,
                        };
                        messages.push(ChatMessage {
                            role,
                            content: db_msg.content,
                            timestamp: Instant::now(),
                            is_system: false,
                        });
                    }
                    self.messages_per_provider.insert(provider.db_name().to_string(), messages);
                }
            }
        }
    }

    fn get_current_messages(&self) -> Vec<ChatMessage> {
        self.messages_per_provider
            .get(self.provider.db_name())
            .cloned()
            .unwrap_or_default()
    }

    fn get_current_messages_mut(&mut self) -> &mut Vec<ChatMessage> {
        self.messages_per_provider
            .entry(self.provider.db_name().to_string())
            .or_insert_with(Vec::new)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('l') => {
                    // Clear current provider's messages
                    self.get_current_messages_mut().clear();
                    self.scroll_offset = 0;

                    if let Some(ref db) = self.db {
                        let _ = db.clear_history(self.provider.db_name());
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::F(1) => {
                self.show_help = !self.show_help;
            }
            KeyCode::F(2) => {
                // Drain any pending responses
                while self.response_rx.try_recv().is_ok() {}

                self.is_streaming = false;

                // Cycle through providers
                self.provider = match self.provider {
                    AIProvider::Claude => AIProvider::Grok,
                    AIProvider::Grok => AIProvider::OpenAI,
                    AIProvider::OpenAI => AIProvider::Gemini,
                    AIProvider::Gemini => AIProvider::Claude,
                };
                self.ai_client = AIClient::new(self.provider.clone());

                // Reset scroll when switching providers
                self.scroll_offset = 0;

                self.add_system_message(&format!("Switched to {}", self.provider.name()));
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Enter => {
                if !self.input_buffer.is_empty() && !self.is_streaming {
                    let user_input = self.input_buffer.clone();
                    self.input_buffer.clear();

                    let messages = self.get_current_messages_mut();
                    messages.push(ChatMessage {
                        role: MessageRole::User,
                        content: user_input.clone(),
                        timestamp: Instant::now(),
                        is_system: false,
                    });

                    if let Some(ref db) = self.db {
                        let _ = db.save_message(self.provider.db_name(), "user", &user_input);
                    }

                    self.is_streaming = true;
                    self.send_message(user_input);
                }
            }
            KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyCode::Down => {
                let msg_count = self.get_current_messages().len();
                if self.scroll_offset < msg_count.saturating_sub(1) {
                    self.scroll_offset += 1;
                }
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                let msg_count = self.get_current_messages().len();
                self.scroll_offset = (self.scroll_offset + 10).min(msg_count.saturating_sub(1));
            }
            _ => {}
        }

        Ok(())
    }

    fn send_message(&mut self, _content: String) {
        let current_messages = self.get_current_messages();
        let messages: Vec<Message> = current_messages
            .iter()
            .filter(|m| !m.is_system)
            .map(|m| Message {
                role: match m.role {
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                },
                content: m.content.clone(),
            })
            .collect();

        let client = self.ai_client.clone();
        let tx = self.response_tx.clone();
        tokio::spawn(async move {
            let result = client.send_message(messages).await;
            let _ = tx.send(result);
        });
    }

    pub fn update(&mut self) -> Result<()> {
        if let Ok(result) = self.response_rx.try_recv() {
            self.is_streaming = false;
            match result {
                Ok(response) => {
                    // Save to database first
                    if let Some(ref db) = self.db {
                        let _ = db.save_message(self.provider.db_name(), "assistant", &response);
                    }

                    // Then add to messages
                    let messages = self.get_current_messages_mut();
                    messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content: response.clone(),
                        timestamp: Instant::now(),
                        is_system: false,
                    });

                    // Auto-scroll to bottom
                    let msg_len = messages.len();
                    self.scroll_offset = msg_len.saturating_sub(1);
                }
                Err(e) => {
                    self.add_system_message(&format!("Error: {}", e));
                }
            }
        }

        Ok(())
    }

    fn add_system_message(&mut self, content: &str) {
        let messages = self.get_current_messages_mut();
        messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: format!("🔧 {}", content),
            timestamp: Instant::now(),
            is_system: true,
        });
    }

    pub fn render(&mut self, frame: &mut Frame) -> Result<()> {
        let area = frame.area();

        // Main layout with semi-transparent panels over video
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),      // Header
                Constraint::Min(5),         // Messages
                Constraint::Length(3),      // Input
                Constraint::Length(1),      // Footer
            ])
            .split(area);

        // Header - semi-transparent
        let header_text = format!("🎬 MEGA-CLI // {} ", self.provider.name());
        let header = Paragraph::new(header_text)
            .style(Style::default().fg(self.provider.color()).bold())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .border_style(Style::default().fg(self.provider.color())),
            );
        frame.render_widget(header, chunks[0]);

        // Messages area
        if self.show_help {
            self.render_help(frame, chunks[1]);
        } else {
            self.render_messages(frame, chunks[1]);
        }

        // Input area
        let input_text = if self.is_streaming {
            "⏳ Waiting for response...".to_string()
        } else {
            format!("> {}_", self.input_buffer)
        };
        let input = Paragraph::new(input_text)
            .style(Style::default().fg(Color::Cyan))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title("Input")
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        frame.render_widget(input, chunks[2]);

        // Footer
        let footer_text = "F1 Help | F2 Switch AI | Ctrl+C Exit | Ctrl+L Clear";
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(footer, chunks[3]);

        Ok(())
    }

    fn render_messages(&self, frame: &mut Frame, area: Rect) {
        let messages = self.get_current_messages();

        if messages.is_empty() {
            let welcome = Paragraph::new(format!(
                "Welcome to MEGA-CLI! 🎬\n\n\
                Connected to: {}\n\n\
                Type your message and press Enter to start.\n\
                The video plays in the background while you chat!\n\n\
                Press F1 for help.",
                self.provider.name()
            ))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).bold())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title("Messages")
                    .border_style(Style::default().fg(Color::White)),
            );
            frame.render_widget(welcome, area);
            return;
        }

        let mut lines = vec![];
        for (idx, msg) in messages.iter().enumerate() {
            if idx < self.scroll_offset {
                continue;
            }

            let (prefix, color) = match msg.role {
                MessageRole::User => ("You", Color::Green),
                MessageRole::Assistant => (
                    self.provider.name(),
                    self.provider.color(),
                ),
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{}: ", prefix), Style::default().fg(color).bold()),
                Span::styled(&msg.content, Style::default().fg(color)),
            ]));

            if idx < messages.len() - 1 {
                lines.push(Line::from(""));
            }
        }

        let messages_text = Text::from(lines);
        let messages_paragraph = Paragraph::new(messages_text)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title("Messages")
                    .border_style(Style::default().fg(Color::White)),
            );

        frame.render_widget(messages_paragraph, area);
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let help_text =
"🎬 MEGA-CLI Keyboard Shortcuts

Navigation:
  ↑/↓         Scroll messages
  PgUp/PgDn   Scroll 10 messages

Commands:
  Enter       Send message
  F1          Toggle this help
  F2          Switch AI provider
  Ctrl+L      Clear conversation
  Ctrl+C      Exit

AI Providers:
  • Claude Sonnet 4
  • Grok 4
  • GPT-5
  • Gemini 2.5 Pro

The animated video background plays continuously
while you chat, creating a cinematic experience!

Your conversations are saved per AI provider.
Switch between providers with F2 - your chat
history will be preserved!

Press F1 to return to chat.";

        let help = Paragraph::new(help_text)
            .alignment(Alignment::Left)
            .style(Style::default().fg(Color::Cyan))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title("Help")
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        frame.render_widget(help, area);
    }
}
