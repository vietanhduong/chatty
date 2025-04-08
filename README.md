# ChaTTY

ChaTTY is a Terminal User Interface (TUI) for chatting with AI models (OpenAI, Gemini), written in Rust.
It brings the power of ChatGPT and Gemini to your terminal, with features like conversation management,
multiple model support, and intelligent context compression.

<div align="center">
<img src="./assets/demo.gif" alt="ChaTTY Demo"/>

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=for-the-badge)](LICENSE)
</div>

## Features
* **Multiple AI Models**: Support for OpenAI and Gemini
* **Conversation Management**: Save and restore chat histories
* **Smart Context Compression**: Automatically manages long conversations (experimental)
* **Syntax Highlighting**: Beautiful code block colorization
* **Fast and Efficient**: Written in Rust for optimal performance

## Quick Start

### Installation
```console
# Download from GitHub https://github.com/vietanhduong/chatty/releases
# or
# Install from Crate.io
$ cargo install --locked chatty-rs

# Build from source
$ git clone https://github.com/vietanhduong/chatty && cd chatty
# The output will be in ./builds/usr/bin/chatty
$ make DESTDIR=builds install
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
alias = "OpenAI"
kind = "openai"
endpoint = "https://api.openai.com"
api_key = "<your_api_key>"
```

View the complete configuration options in our [default config file](./.chatty.default.toml).

## Command Usage
```console
$ chatty --help
A Terminal UI to interact OpenAI models

Default configuration file location looks up in the following order:
    * $XDG_CONFIG_HOME/chatty/config.toml
    * $HOME/.config/chatty/config.toml
    * $HOME/.chatty.toml


Usage: chatty [OPTIONS]

Options:
  -c, --config <PATH>
          Configuration file path

  -v, --version
          Show the version

  -h, --help
          Print help (see a summary with '-h')
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
