pub mod constants;
pub(crate) mod defaults;
pub mod models;
pub mod utils;

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

#[macro_export]
macro_rules! verbose {
    ($($arg:tt)*) => {
        if $crate::config::Configuration::instance().general.verbose {
            eprintln!($($arg)*);
        }
    };
    () => {};
}

pub use verbose;
