use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
};
use std::time::Duration;

mod video;
mod chat;
mod ai;
mod db;

use video::VideoBackground;
use chat::ChatInterface;
use ai::AIProvider;

#[derive(Parser, Debug)]
#[command(name = "MEGA-CLI", about = "Multi-AI terminal chatbot with animated background")]
struct Args {
    /// AI provider to use (claude, grok, gpt, gemini)
    #[arg(long, default_value = "claude")]
    provider: String,

    /// Video background opacity (0.0 - 1.0)
    #[arg(long, default_value = "0.3")]
    opacity: f32,
}

struct App {
    video_bg: VideoBackground,
    chat: ChatInterface,
    should_quit: bool,
}

impl App {
    fn new(provider: AIProvider, opacity: f32) -> Result<Self> {
        // Get terminal size for video scaling
        let size = crossterm::terminal::size()?;
        let video_bg = VideoBackground::new("loading.mp4", size.0, size.1, opacity)?;

        Ok(Self {
            video_bg,
            chat: ChatInterface::new(provider),
            should_quit: false,
        })
    }

    fn handle_input(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                // Global quit handlers
                if key.code == KeyCode::Esc
                    || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
                {
                    self.should_quit = true;
                    return Ok(());
                }

                // Pass to chat interface
                self.chat.handle_key(key)?;
            }
        }
        Ok(())
    }

    fn update(&mut self) -> Result<()> {
        // Update video background (gets next frame)
        self.video_bg.update();

        // Update chat (polls AI responses)
        self.chat.update()?;

        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) -> Result<()> {
        let area = frame.area();

        // First render video background with opacity
        self.video_bg.render_background(frame.buffer_mut(), area);

        // Then render chat interface on top
        self.chat.render(frame)?;

        Ok(())
    }

    fn should_quit(&self) -> bool {
        self.should_quit
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load environment variables
    let _ = dotenvy::dotenv();

    // Parse AI provider
    let provider = match args.provider.to_lowercase().as_str() {
        "claude" => AIProvider::Claude,
        "grok" => AIProvider::Grok,
        "gpt" | "openai" => AIProvider::OpenAI,
        "gemini" => AIProvider::Gemini,
        _ => {
            eprintln!("Unknown provider: {}. Using Claude.", args.provider);
            AIProvider::Claude
        }
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let mut app = App::new(provider, args.opacity)?;

    loop {
        terminal.draw(|f| {
            if let Err(e) = app.render(f) {
                eprintln!("Render error: {}", e);
            }
        })?;

        app.handle_input()?;

        if app.should_quit() {
            break;
        }

        app.update()?;

        // Small sleep to prevent CPU spinning (60 FPS)
        tokio::time::sleep(Duration::from_millis(16)).await;
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    println!("Thanks for using MEGA-CLI! 👋");

    Ok(())
}
