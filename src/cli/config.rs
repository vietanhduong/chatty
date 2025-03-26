#[cfg(test)]
#[path = "config_test.rs"]
mod tests;

use crate::models::Configuration;
use chrono::Local;
use eyre::{Context, Result};
use log::LevelFilter;
use regex::Regex;
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

    let raw_level = log.level().unwrap_or("info");
    let log_level = LevelFilter::from_str(raw_level)?;

    let mut builder = env_logger::Builder::new();

    for filter in log.filters().unwrap_or_default() {
        let module_level =
            LevelFilter::from_str(filter.level().unwrap_or(raw_level)).unwrap_or(log_level.clone());
        builder.filter(Some(filter.module()), module_level);
    }

    builder
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

/// resolve_path resolves the input path to an absolute path. If the
/// input path contains environment variables, it will expand them to their
/// values.
pub fn resolve_path(path: &str) -> Result<String> {
    let re = Regex::new(r"\$\{?([A-Za-z_]+)\}?").wrap_err("compiling regex")?;

    let mut ret = String::new();
    let mut last_pos = 0;

    for cap in re.captures_iter(path) {
        let full_match = cap.get(0).unwrap();
        let start = full_match.start();
        let end = full_match.end();
        ret.push_str(&path[last_pos..start]);
        let var_name = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str())
            .unwrap();

        let var_value = std::env::var(var_name).unwrap_or_default();
        ret.push_str(&var_value);
        last_pos = end;
    }
    ret.push_str(&path[last_pos..]);

    // Resolve the path to an absolute path
    let path = std::path::absolute(ret.as_str()).wrap_err(format!("resolving path {}", ret))?;
    Ok(path.to_string_lossy().to_string())
}
