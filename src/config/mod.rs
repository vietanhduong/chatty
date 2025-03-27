pub mod models;
pub mod utils;

pub use models::*;
pub use utils::*;

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
