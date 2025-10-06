# 🎬 MEGA-CLI - Animated Terminal Chatbot

A stunning multi-AI terminal chatbot with an animated video background that plays continuously while you chat, creating a cinematic 3D-like experience in your terminal!

## ✨ Features

- **🎥 Animated Video Background** - RGB color ASCII video plays continuously with adjustable opacity
- **🤖 Multiple AI Providers** - Switch between:
  - Claude Sonnet 4
  - Grok 4
  - GPT-5
  - Gemini 2.5 Pro
- **💾 Conversation History** - SQLite database persists conversations per AI provider
- **🎨 Beautiful TUI** - Built with Ratatui for a smooth terminal interface
- **⚡ Async Architecture** - Non-blocking AI API calls with Tokio
- **🎮 Interactive Controls** - Keyboard shortcuts for all features

## 🚀 How It Works

The app uses a hybrid architecture that combines:

1. **Video Background Layer** - ffmpeg-next decodes video frames in a separate thread, converts them to RGB ASCII art, and renders with adjustable opacity
2. **Chat Interface Layer** - Ratatui widgets render on top of the video background with semi-transparent panels
3. **AI Communication** - Async HTTP clients communicate with multiple AI provider APIs
4. **Database Persistence** - SQLite stores conversation history for each AI provider

The video loops continuously, creating a flowing animated background while you chat with AI!

## 📦 Installation

### Prerequisites

1. **Rust** - Install from [rust-lang.org](https://www.rust-lang.org/)
2. **FFmpeg** - Required for video processing:
   ```bash
   # macOS
   brew install ffmpeg

   # Ubuntu/Debian
   sudo apt install ffmpeg libavcodec-dev libavformat-dev libavutil-dev libswscale-dev

   # Arch Linux
   sudo pacman -S ffmpeg
   ```

3. **API Keys** - Set up at least one AI provider:
   ```bash
   export CLAUDE_API_KEY="your-key"     # For Claude
   export GROK_API_KEY="your-key"       # For Grok
   export OPENAI_API_KEY="your-key"     # For GPT
   export GEMINI_API_KEY="your-key"     # For Gemini
   ```

### Build

```bash
# macOS (with Homebrew ffmpeg)
export PKG_CONFIG_PATH="/opt/homebrew/opt/ffmpeg/lib/pkgconfig"
cargo build --release

# Linux
cargo build --release
```

## 🎮 Usage

### Basic Usage

```bash
# Run with default settings (Claude, 30% opacity)
cargo run --release

# Or if built
./target/release/animated-cli
```

### Command-line Options

```bash
# Choose AI provider
cargo run --release -- --provider claude
cargo run --release -- --provider grok
cargo run --release -- --provider gpt
cargo run --release -- --provider gemini

# Adjust video background opacity (0.0 - 1.0)
cargo run --release -- --opacity 0.5    # 50% opacity
cargo run --release -- --opacity 0.2    # 20% opacity (subtle)
cargo run --release -- --opacity 0.8    # 80% opacity (prominent)
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| **Enter** | Send message |
| **F1** | Toggle help screen |
| **F2** | Switch AI provider |
| **↑/↓** | Scroll messages |
| **PgUp/PgDn** | Scroll 10 messages |
| **Ctrl+L** | Clear conversation |
| **Ctrl+C** or **Esc** | Exit |

## 📁 Project Structure

```
animated-cli/
├── src/
│   ├── main.rs      # App entry point and main loop
│   ├── video.rs     # Video background with opacity
│   ├── chat.rs      # Chat interface TUI
│   ├── ai.rs        # AI provider APIs
│   └── db.rs        # SQLite database
├── loading.mp4      # Background video file
├── Cargo.toml       # Dependencies
└── README.md        # This file
```

## 🎨 Customization

### Change Background Video

Replace `loading.mp4` with your own video file. The app will automatically:
- Scale it to terminal size
- Convert to RGB ASCII art
- Loop it continuously
- Apply the opacity setting

### Adjust Opacity

The opacity parameter controls how visible the video background is:
- `0.0` - Invisible (chat only)
- `0.3` - Default (subtle background)
- `1.0` - Full brightness (prominent video)

## 🛠️ Technical Details

- **Video Processing**: Uses `ffmpeg-next` for hardware-accelerated video decoding
- **Terminal UI**: Built with `ratatui` and `crossterm`
- **Threading**: Video decoding runs in a separate thread with crossbeam channels
- **AI APIs**: Async HTTP clients with `reqwest` and `tokio`
- **Database**: SQLite with `rusqlite` for conversation persistence
- **Error Handling**: Comprehensive error handling with `anyhow`

## 🐛 Troubleshooting

### Build Errors

**Error: `ffmpeg` not found**
```bash
# Install ffmpeg first
brew install ffmpeg  # macOS
```

**Error: `Unable to generate bindings`**
```bash
# Set PKG_CONFIG_PATH for macOS
export PKG_CONFIG_PATH="/opt/homebrew/opt/ffmpeg/lib/pkgconfig"
cargo build --release
```

### Runtime Errors

**Error: Video file not found**
- Ensure `loading.mp4` exists in the project root
- Or provide your own video file with the same name

**Error: API key not found**
- Set environment variable for your chosen provider
- Example: `export CLAUDE_API_KEY="your-key"`

## 🎯 Future Ideas

- [ ] Multiple video backgrounds you can switch between
- [ ] Custom ASCII character palettes
- [ ] Video playback controls (pause, speed)
- [ ] Streaming AI responses (word-by-word)
- [ ] Export conversations to markdown
- [ ] Custom color schemes per AI provider
- [ ] Image generation display in terminal

## 📝 License

This project is open source and available for personal and educational use.

---

**Enjoy your cinematic AI chat experience!** 🎬✨
