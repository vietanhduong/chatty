#[cfg(test)]
#[path = "model_filter_test.rs"]
mod tests;

use eyre::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum ModelFilter {
    #[serde(rename = "contains")]
    Contains(String),

    #[serde(rename = "equals")]
    Equals(String),

    #[serde(rename = "regex")]
    Regex(String),
}

impl ModelFilter {
    pub fn build(&self) -> Result<regex::Regex> {
        match self {
            ModelFilter::Contains(substring) => {
                let pattern = format!(".*{}.*", regex::escape(substring));
                regex::Regex::new(&pattern)
            }
            ModelFilter::Equals(exact) => {
                let pattern = format!("^{}$", regex::escape(exact));
                regex::Regex::new(&pattern)
            }
            ModelFilter::Regex(pattern) => regex::Regex::new(pattern),
        }
        .wrap_err("building regex")
    }
}
