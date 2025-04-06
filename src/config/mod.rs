pub mod constants;
pub mod defaults;
pub mod model_filter;
pub mod models;
pub mod utils;

use eyre::Result;
pub use models::*;
pub use utils::*;

#[cfg(test)]
use std::cell::RefCell;

use std::sync::OnceLock;

pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_SHA: &str = match option_env!("GIT_SHA") {
    Some(v) => v,
    None => "unknown",
};

pub fn user_agent() -> String {
    format!("{}/{}", APP_NAME, VERSION)
}

pub fn version() -> String {
    format!("{} version: {} {}", APP_NAME, VERSION, GIT_SHA)
}

#[allow(dead_code)]
static CONFIG: OnceLock<Configuration> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static TEST_CONFIG: RefCell<&'static Configuration> = RefCell::new(Box::leak(Box::new(Configuration::default())))
}

#[cfg(not(test))]
pub fn instance() -> &'static Configuration {
    CONFIG.get().expect("Config not initialized")
}

#[cfg(test)]
pub fn instance() -> &'static Configuration {
    TEST_CONFIG.with(|config| *config.borrow())
}

pub fn init(config: Configuration) -> Result<()> {
    #[cfg(not(test))]
    CONFIG
        .set(config)
        .map_err(|_| eyre::eyre!("Config already initialized"))?;
    #[cfg(test)]
    TEST_CONFIG.with(|test_config| {
        *test_config.borrow_mut() = Box::leak(Box::new(config));
    });
    Ok(())
}

#[macro_export]
macro_rules! verbose {
    ($($arg:tt)*) => {
        if $crate::config::instance().general.verbose {
            eprintln!($($arg)*);
        }
    };
    () => {};
}

pub use verbose;
