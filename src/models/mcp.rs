use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: ToolInputSchema,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ToolInputSchema {
    #[serde(rename = "type")]
    pub type_name: String,
    pub properties: HashMap<String, ToolInputSchemaProperty>,
    pub required: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ToolInputSchemaProperty {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
