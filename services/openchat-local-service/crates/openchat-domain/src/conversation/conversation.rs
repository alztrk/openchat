//! Core conversation types.

/// Unique identifier for a conversation.
pub struct ConversationId(pub String);

/// Unique identifier for a message.
pub struct MessageId(pub String);

/// Represents the current state of a conversation.
pub struct ConversationState(pub u32);
