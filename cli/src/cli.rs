use clap::Parser;
use eyre::{Context, Result};
use openai_models::config::Configuration;

use crate::load_configuration;

#[derive(Debug, Parser)]
#[command(
    version,
    about,
    long_about = r#"A Terminal UI to interact OpenAI models

Default configuration file location looks up in the following order:
    * $XDG_CONFIG_HOME/openai-tui/config.toml
    * $HOME/.config/openai-tui/config.toml
    * $HOME/.openai-tui.toml
"#
)]
pub struct Command {
    /// Configuration file path
    #[arg(short, long, value_name = "PATH")]
    pub config: Option<String>,
}

impl Command {
    pub fn get_config() -> Result<Configuration> {
        let cmd = Self::parse();

        let config_path = cmd
            .config
            .clone()
            .unwrap_or_else(|| lookup_config_path().unwrap_or_default());

        if config_path.is_empty() {
            // No config path is specified just use the default config
            return Ok(Configuration::default());
        }
        Ok(load_configuration(config_path.as_str()).wrap_err("loading configuration")?)
    }
}

/// lookup_config_path trys to look up the config path at:
/// * $XDG_CONFIG_HOME/openai-tui/config.toml
/// * $HOME/.config/openai-tui/config.toml
/// * $HOME/.openai-tui.toml
fn lookup_config_path() -> Option<String> {
    let paths = &[
        format!(
            "{}/.config/openai-tui/config.toml",
            env_or_current("XDG_CONFIG_HOME")
        ),
        format!("{}/.config/openai-tui/config.toml", env_or_current("HOME")),
        format!("{}/.openai-tui.toml", env_or_current("HOME")),
    ];

    for path in paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

fn env_or_current(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| ".".to_string())
}
