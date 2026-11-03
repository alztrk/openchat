//! Context-usage estimates used by the chat UI.
//!
//! Provider-reported input usage is reused only when it belongs to the same
//! provider route and a message boundary still present in the conversation.
//! New content is estimated separately so drafts and pending attachments are
//! not omitted or counted as part of the previous provider measurement.

use std::collections::BTreeMap;

use super::token_estimator;
use crate::chatgpt_store::{ConversationContextState, StoredMessage};
use crate::context_compaction::{compaction_payload_matches_route, text_token_estimate};

const IMAGE_TOKEN_ESTIMATE: i64 = 2048;
const OPAQUE_CHECKPOINT_TOKEN_ESTIMATE: i64 = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingAttachmentEstimate {
    pub name: String,
    pub kind: String,
    pub size_bytes: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ContextUsageAttachment {
    pub name: String,
    pub tokens: i64,
    pub is_draft: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ToolUsageEstimate {
    pub name: String,
    pub tokens: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FullContextUsageEstimate {
    pub total_tokens: i64,
    pub user_message_tokens: i64,
    pub assistant_message_tokens: i64,
    pub attachments: Vec<ContextUsageAttachment>,
    pub tool_usage: Vec<ToolUsageEstimate>,
    pub memory_tokens: i64,
    pub draft_tokens: i64,
    pub provider_input_tokens: Option<i64>,
    pub usage_mode: &'static str,
    pub estimate_source: &'static str,
}

#[derive(Default)]
struct MessageBreakdown {
    user_tokens: i64,
    assistant_tokens: i64,
    attachments: Vec<ContextUsageAttachment>,
    tools: BTreeMap<String, i64>,
}

impl MessageBreakdown {
    fn total_tokens(&self) -> i64 {
        self.user_tokens
            .saturating_add(self.assistant_tokens)
            .saturating_add(
                self.attachments
                    .iter()
                    .map(|attachment| attachment.tokens)
                    .fold(0, i64::saturating_add),
            )
            .saturating_add(self.tools.values().copied().fold(0, i64::saturating_add))
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn estimate_full_context_usage(
    model_id: &str,
    provider_id: Option<&str>,
    connection_id: Option<&str>,
    workspace_id: Option<&str>,
    messages: &[StoredMessage],
    state: Option<&ConversationContextState>,
    draft: &str,
    pending_attachments: &[PendingAttachmentEstimate],
) -> FullContextUsageEstimate {
    let estimate_source = token_estimator::source_for_model(model_id).as_str();

    let compaction_matches_route = state.is_some_and(|state| {
        compaction_payload_matches_route(
            state,
            provider_id.unwrap_or_default(),
            connection_id,
            workspace_id,
            model_id,
        )
    });
    let boundary_index = state
        .filter(|_| compaction_matches_route)
        .and_then(|state| state.compacted_through_message_id.as_deref())
        .and_then(|message_id| messages.iter().position(|message| message.id == message_id));
    let active_start = boundary_index.map_or(0, |index| index.saturating_add(1));
    let active_messages = &messages[active_start..];
    let provider_input_tokens = matching_provider_input_tokens(
        model_id,
        provider_id,
        connection_id,
        workspace_id,
        messages,
        state,
        boundary_index,
    );
    let accounting_messages = provider_input_tokens.map_or(active_messages, |(_, prompt_index)| {
        &messages[prompt_index.saturating_add(1)..]
    });
    let breakdown = estimate_messages(model_id, accounting_messages);

    let mut attachments = breakdown.attachments.clone();
    attachments.extend(
        pending_attachments
            .iter()
            .map(|attachment| ContextUsageAttachment {
                name: attachment.name.clone(),
                tokens: estimate_pending_attachment(model_id, attachment),
                is_draft: true,
            }),
    );

    let memory_tokens = if provider_input_tokens.is_some() || !compaction_matches_route {
        0
    } else {
        estimate_memory(model_id, state)
    };
    let draft_tokens = text_token_estimate(model_id, draft);
    let pending_tokens = pending_attachments
        .iter()
        .map(|attachment| estimate_pending_attachment(model_id, attachment))
        .fold(0, i64::saturating_add);

    let total_tokens = if let Some((input_tokens, _)) = provider_input_tokens {
        input_tokens
            .saturating_add(breakdown.total_tokens())
            .saturating_add(draft_tokens)
            .saturating_add(pending_tokens)
    } else {
        breakdown
            .total_tokens()
            .saturating_add(memory_tokens)
            .saturating_add(draft_tokens)
            .saturating_add(pending_tokens)
    };

    let tool_usage = breakdown
        .tools
        .into_iter()
        .map(|(name, tokens)| ToolUsageEstimate { name, tokens })
        .collect();

    FullContextUsageEstimate {
        total_tokens,
        user_message_tokens: breakdown.user_tokens,
        assistant_message_tokens: breakdown.assistant_tokens,
        attachments,
        tool_usage,
        memory_tokens,
        draft_tokens,
        provider_input_tokens: provider_input_tokens.map(|(tokens, _)| tokens),
        usage_mode: if provider_input_tokens.is_some() {
            "provider_reported_plus_estimate"
        } else {
            "estimated"
        },
        estimate_source,
    }
}

fn matching_provider_input_tokens(
    model_id: &str,
    provider_id: Option<&str>,
    connection_id: Option<&str>,
    workspace_id: Option<&str>,
    messages: &[StoredMessage],
    state: Option<&ConversationContextState>,
    boundary_index: Option<usize>,
) -> Option<(i64, usize)> {
    let state = state?;
    let input_tokens = state.last_prompt_tokens.filter(|tokens| *tokens >= 0)?;
    if state.last_prompt_provider_id.as_deref() != provider_id
        || state.last_prompt_model_id.as_deref() != Some(model_id)
        || state.last_prompt_connection_id.as_deref() != connection_id
        || state.last_prompt_workspace_id.as_deref() != workspace_id
    {
        return None;
    }

    let message_id = state.last_prompt_message_id.as_deref()?;
    let prompt_index = messages
        .iter()
        .position(|message| message.id == message_id)?;
    // The compacted-through message is no longer part of the provider request.
    if boundary_index.is_some_and(|boundary| prompt_index <= boundary) {
        return None;
    }
    Some((input_tokens, prompt_index))
}

fn estimate_messages(model_id: &str, messages: &[StoredMessage]) -> MessageBreakdown {
    let mut result = MessageBreakdown::default();
    for message in messages {
        let message_tokens = text_token_estimate(model_id, &message.content);

        if message.role == "user" {
            result.user_tokens = result.user_tokens.saturating_add(message_tokens);
        } else {
            result.assistant_tokens = result.assistant_tokens.saturating_add(message_tokens);
        }

        for attachment in &message.attachments {
            let tokens = if attachment.kind == "image" {
                IMAGE_TOKEN_ESTIMATE
            } else if let Some(content) = attachment.content.as_deref() {
                text_token_estimate(model_id, &String::from_utf8_lossy(content))
            } else {
                text_token_estimate(model_id, &attachment.name)
            };
            result.attachments.push(ContextUsageAttachment {
                name: attachment.name.clone(),
                tokens,
                is_draft: false,
            });
        }

        for activity in message
            .tool_activities
            .iter()
            .filter(|activity| activity.has_completed_result())
        {
            let arguments = serde_json::to_string(&activity.arguments).unwrap_or_default();
            let output = activity
                .output
                .as_ref()
                .and_then(|value| value.as_str().map(str::to_owned))
                .or_else(|| activity.output.as_ref().map(ToString::to_string))
                .unwrap_or_default();
            let activity_tokens = text_token_estimate(model_id, &activity.name)
                .saturating_add(text_token_estimate(model_id, &arguments))
                .saturating_add(text_token_estimate(model_id, &output));
            let total = result.tools.entry(activity.name.clone()).or_default();
            *total = total.saturating_add(activity_tokens);
        }
    }
    result
}

fn estimate_pending_attachment(model_id: &str, attachment: &PendingAttachmentEstimate) -> i64 {
    if attachment.kind == "image" {
        return IMAGE_TOKEN_ESTIMATE;
    }
    let size = usize::try_from(attachment.size_bytes.max(0)).unwrap_or(usize::MAX);
    let size_estimate = i64::try_from(size.div_ceil(4)).unwrap_or(i64::MAX);
    let name_estimate = text_token_estimate(model_id, &attachment.name);
    size_estimate.max(name_estimate)
}

fn estimate_memory(model_id: &str, state: Option<&ConversationContextState>) -> i64 {
    let Some(state) = state else {
        return 0;
    };
    match state.compaction_kind.as_deref() {
        Some("summary") => state
            .compaction_payload
            .as_deref()
            .map(|summary| text_token_estimate(model_id, summary))
            .unwrap_or(0),
        Some(
            "responses_checkpoint"
            | "openai_responses_item"
            | "anthropic_compaction_block"
            | "xai_responses_item",
        ) => OPAQUE_CHECKPOINT_TOKEN_ESTIMATE,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingAttachmentEstimate, estimate_full_context_usage, text_token_estimate};
    use crate::chatgpt_store::{ConversationContextState, StoredMessage};

    fn message(id: &str, role: &str, content: &str) -> StoredMessage {
        StoredMessage {
            id: id.to_owned(),
            role: role.to_owned(),
            content: content.to_owned(),
            status: "completed".to_owned(),
            output_tokens: None,
            tool_activities: Vec::new(),
            attachments: Vec::new(),
        }
    }

    fn state() -> ConversationContextState {
        ConversationContextState {
            compaction_kind: None,
            compaction_payload: None,
            compaction_provider_id: None,
            compaction_connection_id: None,
            compaction_workspace_id: None,
            compaction_model_id: None,
            compacted_through_message_id: None,
            last_prompt_tokens: Some(20),
            last_prompt_message_id: Some("prompt".to_owned()),
            last_prompt_provider_id: Some("provider".to_owned()),
            last_prompt_model_id: Some("gpt-4.1".to_owned()),
            last_prompt_connection_id: Some("connection".to_owned()),
            last_prompt_workspace_id: Some("workspace".to_owned()),
        }
    }

    #[test]
    fn previous_provider_usage_adds_only_new_messages_draft_and_pending_attachments() {
        let mut reply = message("reply", "assistant", "new reply");
        reply.output_tokens = Some(900);
        let messages = [message("prompt", "user", "sent prompt"), reply];
        let pending = [PendingAttachmentEstimate {
            name: "image.png".to_owned(),
            kind: "image".to_owned(),
            size_bytes: 1024,
        }];
        let estimate = estimate_full_context_usage(
            "gpt-4.1",
            Some("provider"),
            Some("connection"),
            Some("workspace"),
            &messages,
            Some(&state()),
            "new draft",
            &pending,
        );

        let new_reply_tokens = text_token_estimate("gpt-4.1", "new reply");
        let draft_tokens = text_token_estimate("gpt-4.1", "new draft");
        assert_eq!(estimate.provider_input_tokens, Some(20));
        assert_eq!(estimate.usage_mode, "provider_reported_plus_estimate");
        assert_eq!(estimate.user_message_tokens, 0);
        assert_eq!(estimate.assistant_message_tokens, new_reply_tokens);
        assert_eq!(estimate.memory_tokens, 0);
        assert_eq!(estimate.attachments.len(), 1);
        assert_eq!(estimate.attachments[0].tokens, 2048);
        assert_eq!(
            estimate.total_tokens,
            20 + new_reply_tokens + draft_tokens + 2048
        );
    }

    #[test]
    fn provider_usage_is_not_reused_at_the_compaction_boundary() {
        let mut context_state = state();
        context_state.compacted_through_message_id = Some("prompt".to_owned());
        let messages = [
            message("prompt", "user", "compacted prompt"),
            message("reply", "assistant", "retained reply"),
        ];

        assert_eq!(
            super::matching_provider_input_tokens(
                "gpt-4.1",
                Some("provider"),
                Some("connection"),
                Some("workspace"),
                &messages,
                Some(&context_state),
                Some(0),
            ),
            None,
        );
    }

    #[test]
    fn provider_usage_is_not_reused_for_a_different_route() {
        let messages = [message("prompt", "user", "sent prompt")];
        let estimate = estimate_full_context_usage(
            "gpt-4.1",
            Some("other-provider"),
            Some("connection"),
            Some("workspace"),
            &messages,
            Some(&state()),
            "",
            &[],
        );
        assert_eq!(estimate.provider_input_tokens, None);
        assert_eq!(estimate.usage_mode, "estimated");
    }

    #[test]
    fn compacted_messages_before_boundary_do_not_contribute_to_estimate() {
        let messages = [
            message("old", "user", &"old ".repeat(100)),
            message("boundary", "assistant", "summary checkpoint"),
            message("new", "user", "new question"),
        ];
        let mut context_state = state();
        context_state.compaction_kind = Some("summary".to_owned());
        context_state.last_prompt_tokens = None;
        context_state.last_prompt_message_id = None;
        context_state.compacted_through_message_id = Some("boundary".to_owned());

        let estimate = estimate_full_context_usage(
            "gpt-4.1",
            Some("provider"),
            Some("connection"),
            Some("workspace"),
            &messages,
            Some(&context_state),
            "",
            &[],
        );
        let without_old = estimate_full_context_usage(
            "gpt-4.1",
            Some("provider"),
            Some("connection"),
            Some("workspace"),
            &messages[2..],
            None,
            "",
            &[],
        );
        assert_eq!(estimate.total_tokens, without_old.total_tokens);
    }

    #[test]
    fn route_mismatched_compaction_does_not_hide_messages_or_add_checkpoint_tokens() {
        let messages = [
            message("old", "user", "older message"),
            message("boundary", "assistant", "checkpoint"),
            message("new", "user", "new question"),
        ];
        let mut stale_compaction = state();
        stale_compaction.compaction_kind = Some("responses_checkpoint".to_owned());
        stale_compaction.compaction_payload = Some("opaque checkpoint".to_owned());
        stale_compaction.compaction_provider_id = Some("other-provider".to_owned());
        stale_compaction.compaction_connection_id = Some("other-connection".to_owned());
        stale_compaction.compaction_workspace_id = Some("other-workspace".to_owned());
        stale_compaction.compaction_model_id = Some("other-model".to_owned());
        stale_compaction.compacted_through_message_id = Some("boundary".to_owned());
        stale_compaction.last_prompt_tokens = None;
        stale_compaction.last_prompt_message_id = None;

        let with_stale_compaction = estimate_full_context_usage(
            "gpt-4.1",
            Some("provider"),
            Some("connection"),
            Some("workspace"),
            &messages,
            Some(&stale_compaction),
            "",
            &[],
        );
        let without_compaction = estimate_full_context_usage(
            "gpt-4.1",
            Some("provider"),
            Some("connection"),
            Some("workspace"),
            &messages,
            None,
            "",
            &[],
        );

        assert_eq!(
            with_stale_compaction.total_tokens,
            without_compaction.total_tokens
        );
        assert_eq!(with_stale_compaction.memory_tokens, 0);
    }
}
