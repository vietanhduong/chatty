use chrono::Local;
use eyre::{Context, Result};
use log::LevelFilter;
use openai_models::configuration::Configuration;
use std::{io::Write, str::FromStr};
use syntect::highlighting::{Theme, ThemeSet};

pub fn load_configuration(config_path: &str) -> Result<Configuration> {
    let config =
        std::fs::read_to_string(config_path).wrap_err(format!("reading {}", config_path))?;
    let config: Configuration = toml::from_str(&config).wrap_err("parsing configuration")?;
    Ok(config)
}

pub fn init_logger(config: &Configuration) -> Result<()> {
    let log = config.log().cloned().unwrap_or_default();

    let log_file: Box<dyn std::io::Write + Send + 'static> = if let Some(file) = log.file() {
        Box::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(file.append())
                .open(file.path())
                .wrap_err(format!("opening log file {}", file.path()))?,
        )
    } else {
        Box::new(std::io::stderr())
    };

    let log_level = LevelFilter::from_str(log.level().unwrap_or("info"))?;

    env_logger::Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{}/{}:{} {} [{}] - {}",
                record.module_path().unwrap_or("unknown"),
                basename(record.file().unwrap_or("unknown")),
                record.line().unwrap_or(0),
                Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
                record.level(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(log_file))
        .filter(None, log_level)
        .try_init()?;

    Ok(())
}

pub fn init_theme(config: &Configuration) -> Result<Theme> {
    let theme = config.theme().cloned().unwrap_or_default();
    let themes = match theme.folder_path() {
        Some(path) => {
            ThemeSet::load_from_folder(path).wrap_err(format!("loading theme from {}", path))?
        }
        None => syntect::highlighting::ThemeSet::load_defaults(),
    };

    let theme_name = theme.name().unwrap_or_default();
    let theme = themes
        .themes
        .get(theme_name)
        .ok_or_else(|| eyre::eyre!("theme {} not found", theme_name))?;
    Ok(theme.clone())
}

pub fn basename(path: &str) -> String {
    path.split('/').last().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_configuration() {
        let config = load_configuration("./testdata/config.toml").expect("failed to load config");
        assert_eq!(config.log().unwrap().level(), Some("debug"));

        let log_file = config.log().unwrap().file();
        assert!(log_file.is_some());
        assert_eq!(log_file.unwrap().path(), "/var/log/openai-tui.log");
        assert_eq!(log_file.unwrap().append(), true);

        assert_eq!(config.theme().unwrap().name(), Some("dark"));
        assert_eq!(
            config.theme().unwrap().folder_path(),
            Some("/etc/openai-tui/bin/them.bin")
        );

        let backend = config.backend().unwrap();
        assert_eq!(
            backend.openai().unwrap().endpoint(),
            Some("https://api.deepseek.com")
        );

        let models = backend.models().unwrap();
        let expected_models = vec!["deepseek-chat".to_string(), "deepseek-reasoner".to_string()];
        assert_eq!(models, expected_models);
    }
}
