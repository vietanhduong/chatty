# ChaTTY

ChaTTY is a Terminal User Interface (TUI) for chatting with AI models (OpenAI, Gemini), written in Rust.
It brings the power of ChatGPT and Gemini to your terminal, with features like conversation management,
multiple model support, and intelligent context compression.

<div align="center">
![ChaTTY Demo](./assets/demo.gif)

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=for-the-badge)](LICENSE)
</div>

## Features
* **Multiple AI Models**: Support for OpenAI and Gemini
* **Conversation Management**: Save and restore chat histories
* **Smart Context Compression**: Automatically manages long conversations (experimental)
* **Syntax Highlighting**: Beautiful code block colorization
* **Fast and Efficient**: Written in Rust for optimal performance
* **Secure**: Local conversation storage with encryption support

## Quick Start

### Installation
```bash
# Download from Git
# or
# Build from source
git clone https://github.com/vietanhduong/chatty && cd chatty

# The output will be in ./builds/usr/bin/chatty
make DESTDIR=builds install
```

### Configuration
ChaTTY requires minimal configuration before first use. You'll need to provide at least one backend connection.

Create a config file in one of these locations:
* `$XDG_CONFIG_HOME/chatty/config.toml`
* `$HOME/.config/chatty/config.toml`
* `$HOME/.chatty.toml`

#### Basic Configuration Example:
```toml
[[backend.connections]]
enabled = true
alias = "DeepSeek"
kind = "openai"
endpoint = "https://api.deepseek.com"
api_key = "<your_api_key>"
```

View the complete configuration options in our [default config file](./.chatty.default.toml).

## Basic Commands
```bash
chatty                  # Start a new chat session
chatty -c <file>       # Specify a config file
chatty --help          # Show help message
```

## Contributing
Contributions are welcome! Feel free to:
- Report bugs
- Suggest new features
- Submit pull requests

## License
This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments
- Built with [ratatui](https://github.com/ratatui-org/ratatui)
- Inspired by ChatGPT and other terminal-based tools
