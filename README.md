# zero-code-cli

A concise, high-performance terminal coding agent written in safe Rust. It interacts with the DeepSeek API to help you explore, plan, and write code — all from your terminal.

> ~3100 lines of Rust, zero `unsafe` code, single-threaded async runtime.

## Features

- **Dual-mode workflow** — Plan mode for research and design thinking, Build mode for writing code. Switch with `Tab`.
- **Plan artifact handoff** — When you switch from Plan to Build, the plan conversation is captured and injected as context so the Build agent inherits the full design.
- **ReAct agent loop** — The agent reasons, calls tools, and iterates up to 10 turns per message.
- **API retry with exponential backoff** — Failed API calls are retried up to `retry_count` times with configurable delay.
- **Built-in tools** — `read_file`, `write_file` (both with partial read/write via line ranges), `bash` (with timeout enforcement), `grep`, `ls` — all defined with JSON Schema and accessible to the model.
- **Streaming TUI** — Real-time token streaming with blinking cursor indicator, rendered with [Ratatui](https://ratatui.rs/).
- **DeepSeek reasoning support** — Handles `reasoning_content` tokens from DeepSeek reasoning models.
- **Configurable** — API endpoint, model, temperature, max tokens, retry settings, and custom system prompt all set via `~/.zero-code-cli/config.toml`.
- **Debug logging** — Set `DEBUG=true` for detailed logs to `~/.zero-code-cli/debug.log`.

## Requirements

- Rust toolchain (edition 2024)
- A [DeepSeek API key](https://platform.deepseek.com/)

## Installation

```bash
git clone https://github.com/your-username/zero-code-cli.git
cd zero-code-cli
cargo build --release
```

The binary will be at `target/release/zero-code-cli`.

## Configuration

Create `~/.zero-code-cli/config.toml`:

```toml
api_url = "https://api.deepseek.com"
api_key = "sk-your-key-here"
model = "deepseek-v4-flash"
max_tokens = 4096
temperature = 0.7
retry_count = 2
retry_delay_secs = 2
system_prompt = "You are a helpful coding assistant."
```

Environment variable overrides:

| Variable | Config key |
|---|---|
| `DEEPSEEK_API_KEY` | `api_key` |
| `DEEPSEEK_API_URL` | `api_url` |
| `DEEPSEEK_MODEL` | `model` |

## Usage

```bash
cargo run
# or with debug logging
DEBUG=true cargo run
```

### Keybindings

| Key | Action |
|---|---|
| `Enter` | Send message (or handle slash command) |
| `Tab` | Switch Plan ↔ Build mode |
| `Ctrl+C` / `Ctrl+D` | Quit |
| `Ctrl+W` | Delete previous word |
| `Home` / `End` | Move to line start/end |
| `Up` / `Down` | Scroll conversation (1 line) |
| `PageUp` / `PageDown` | Scroll conversation (5 lines) |

### Slash Commands

| Command | Action |
|---|---|
| `/new` | Reset both Plan and Build conversations |
| `/plan` | Switch to Plan mode |
| `/build` | Switch to Build mode (captures plan artifact) |

### Workflow

1. **Plan mode** (`/plan`) — Ask the agent to research, explore, and design a solution. The system prompt guides it toward analysis and design, not code writing.
2. **Switch** (`Tab`) — All agent messages from Plan are captured into a plan artifact.
3. **Build mode** (`/build`) — On your first message, the plan artifact is injected as context. The Build system prompt focuses the agent on implementation.
4. **Iterate** — Switch back to Plan anytime to refine the design, then back to Build to continue coding.

### Available Tools

The agent can call these tools on your filesystem:

- `read_file` — Read file contents, supports partial reads via `start_line`/`end_line` (1 MB limit)
- `write_file` — Write or overwrite a file, supports targeted edits via `start_line`/`end_line` (path traversal guarded)
- `bash` — Execute shell commands with configurable timeout (default 30s, max 120s)
- `grep` — Search files by regex (output truncated to 100 KB)
- `ls` — List directory contents

## Examples

See the [`examples/`](examples/) directory for projects built with zero-code-cli:

- **[tetris](examples/tetris/)** — Classic Tetris game (vanilla JS + HTML5 Canvas), generated entirely by the agent.

## Architecture

```
src/
├── main.rs     Entry point, terminal setup, event loop, agent_loop(), key handling
├── app.rs      App state, dual message histories, plan artifact, slash commands
├── api.rs      DeepSeek API client, SSE streaming, tool-call parsing
├── ui.rs       Ratatui rendering: tabs, conversation, input, status bar
├── tools.rs    5 built-in tools with JSON Schema definitions
├── config.rs   Config loading from TOML + env var overrides
└── logger.rs   Debug logging to file
```

**Data flow:** user types → `Enter` spawns `agent_loop()` as a tokio task → `api::stream_chat()` POSTs to the API → SSE tokens stream through an mpsc channel → main event loop drains them into `App` → `ui::draw()` re-renders at ~60fps. When the model responds with tool calls, `agent_loop()` executes them, feeds results back, and loops (max 10 iterations).

## License

MIT
