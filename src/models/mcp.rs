use rpc_router::RpcParams;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, hash::Hash};

#[derive(Deserialize, Serialize, RpcParams, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<CallToolResultContent>,
    #[serde(default)] // This will default to false if missing
    pub is_error: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CallToolResultContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    #[serde(rename = "resource")]
    Resource { resource: ResourceContent },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContent {
    pub uri: String, // The URI of the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>, // Optional MIME type
    pub text: Option<String>, // For text resources
    pub blob: Option<String>, // For binary resources (base64 encoded)
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: ToolInputSchema,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ToolInputSchema {
    #[serde(rename = "type")]
    pub type_name: String,
    pub properties: HashMap<String, ToolInputSchemaProperty>,
    pub required: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
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

impl Hash for Tool {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Eq for Tool {
    fn assert_receiver_is_total_eq(&self) {}
}

impl PartialEq for Tool {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
