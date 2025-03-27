use clap::Parser;
use eyre::{Context, Result};

use crate::config::{self, Configuration, load_configuration, lookup_config_path};

#[derive(Debug, Parser)]
#[command(
    version,
    about,
    long_about = r#"A Terminal UI to interact OpenAI models

Default configuration file location looks up in the following order:
    * $XDG_CONFIG_HOME/chatty/config.toml
    * $HOME/.config/chatty/config.toml
    * $HOME/.chatty.toml
"#,
    disable_version_flag = true
)]
pub struct Command {
    /// Configuration file path
    #[arg(short, long, value_name = "PATH")]
    config: Option<String>,

    /// Show the version
    #[arg(short, long)]
    version: bool,
}

impl Command {
    pub fn new() -> Command {
        Self::parse()
    }

    pub fn get_config(&self) -> Result<Configuration> {
        let config_path = self
            .config
            .clone()
            .unwrap_or_else(|| lookup_config_path().unwrap_or_default());

        if config_path.is_empty() {
            // No config path is specified just use the default config
            return Ok(Configuration::default());
        }
        Ok(load_configuration(config_path.as_str()).wrap_err("loading configuration")?)
    }

    pub fn version(&self) -> bool {
        self.version
    }

    pub fn print_version(&self) {
        println!("{}", config::version())
    }
}
