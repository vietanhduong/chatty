use super::{BackendPrompt, Context, Conversation, Message};

pub enum Action {
    BackendAbort,
    BackendRequest(BackendPrompt),

    SetConversation(String),
    UpsertConversation(UpsertConvoRequest),
    DeleteConversation(String),           // Conversation ID
    UpsertMessage(String, Message),       // Conversation ID, Message
    UpsertConvoContext(String, Context),  // Conversation ID, Context
    DeleteMessage(String),                // Message ID
    CompressConversation(String, String), // Conversation ID, Model ID

    CopyMessages(Vec<Message>),
    CopyText { content: String, notice: bool },
}

pub struct UpsertConvoRequest {
    pub convo: Conversation,
    pub include_messages: bool,
    pub include_context: bool,
}
