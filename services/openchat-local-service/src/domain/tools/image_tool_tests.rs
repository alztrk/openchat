//! Tests for tool definition catalogs and profile allowlists.

use super::{
    ToolDefinition, apply_profile_tool_allowlist, context_usage_definitions, definitions,
    definitions_for_chatgpt_api, definitions_for_chatgpt_model, definitions_for_model,
    definitions_for_provider, definitions_for_request, disable_tools_for_plan_mode,
    model_supports_image_generation, parse_profile_tool_allowlist,
};

#[test]
fn profile_tool_allowlist_distinguishes_inherit_from_empty_and_rejects_unknown_tools() {
    assert!(
        parse_profile_tool_allowlist(None)
            .expect("missing allowlist")
            .is_none()
    );
    let empty = parse_profile_tool_allowlist(Some(&serde_json::json!([])))
        .expect("empty allowlist")
        .expect("restricted allowlist");
    assert!(empty.is_empty());

    let selected = parse_profile_tool_allowlist(Some(&serde_json::json!([
        "web_search",
        "read_file",
        "list_files",
        "search_files"
    ])))
    .expect("valid allowlist")
    .expect("restricted allowlist");
    assert_eq!(selected.len(), 4);
    assert!(selected.contains("web_search"));
    assert!(selected.contains("read_file"));
    assert!(selected.contains("list_files"));
    assert!(selected.contains("search_files"));

    let mcp = parse_profile_tool_allowlist(Some(&serde_json::json!(["mcp__workspace__read"])))
        .expect("valid MCP tool name")
        .expect("restricted allowlist");
    assert!(mcp.contains("mcp__workspace__read"));

    for invalid in [
        serde_json::json!(["web_search", "web_search"]),
        serde_json::json!([42]),
    ] {
        let error = parse_profile_tool_allowlist(Some(&invalid)).expect_err("invalid allowlist");
        assert_eq!(error.code, "invalid_profile_tool_allowlist");
    }
}

#[test]
fn profile_tool_allowlist_only_advertises_selected_tools() {
    let mut tools = definitions();
    let selected = parse_profile_tool_allowlist(Some(&serde_json::json!(["web_search"])))
        .expect("valid allowlist");

    apply_profile_tool_allowlist(&mut tools, selected.as_ref());

    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        ["web_search"]
    );
}

#[test]
fn plan_mode_only_advertises_todo_tools() {
    let mut tools = definitions_for_chatgpt_model();
    tools.push(ToolDefinition {
        name: "mcp__server__write".to_owned(),
        description: "MCP tool".to_owned(),
        parameters: serde_json::json!({"type": "object"}),
    });
    tools.extend(crate::goals::control_tool_definitions());

    disable_tools_for_plan_mode(&mut tools);

    assert!(!tools.is_empty());
    assert!(tools.iter().all(|tool| tool.name.starts_with("todo_")));
    assert!(tools.iter().any(|tool| tool.name == "todo_create"));
}

#[test]
fn image_generation_is_available_when_supported() {
    assert!(
        definitions()
            .iter()
            .any(|tool| tool.name == "generate_image")
    );
    assert!(
        definitions_for_provider("chatgpt")
            .iter()
            .any(|tool| tool.name == "generate_image")
    );
    assert!(
        definitions_for_chatgpt_api()
            .iter()
            .any(|tool| tool.name == "generate_image")
    );
    let api_image_tool = definitions_for_chatgpt_api()
        .into_iter()
        .find(|tool| tool.name == "generate_image")
        .expect("API image tool");
    assert_eq!(
        api_image_tool.parameters["properties"]["count"]["maximum"],
        3
    );
    assert!(model_supports_image_generation("chatgpt", "gpt-image-2"));
    assert!(model_supports_image_generation(
        "gemini",
        "imagen-3.0-generate-002"
    ));
    assert!(!model_supports_image_generation(
        "claude",
        "claude-3-5-sonnet-20241022"
    ));
}

#[test]
fn delegated_child_runs_are_available_on_both_responses_routes() {
    for provider_id in ["chatgpt", "chatgpt_api"] {
        assert!(
            definitions_for_request(provider_id, Some(true))
                .iter()
                .any(|tool| tool.name == "delegate_task"),
            "{provider_id}"
        );
        assert!(definitions_for_request(provider_id, Some(false)).is_empty());
    }

    assert!(
        !definitions_for_provider("openrouter")
            .iter()
            .any(|tool| tool.name == "delegate_task")
    );
}

#[test]
fn image_generation_is_independent_from_image_input_capability() {
    assert!(
        definitions_for_chatgpt_model()
            .iter()
            .any(|tool| tool.name == "generate_image")
    );
    let image_tool = definitions_for_chatgpt_model()
        .into_iter()
        .find(|tool| tool.name == "generate_image")
        .expect("OAuth image tool");
    assert!(image_tool.parameters["properties"].get("count").is_none());
}

#[test]
fn model_capability_controls_provider_tools_and_ask_user_availability() {
    let supported = definitions_for_model("opencode", Some(true));
    assert!(supported.iter().any(|tool| tool.name == "ask_user"));

    let unsupported = definitions_for_model("opencode", Some(false));
    assert!(unsupported.is_empty());
    assert!(context_usage_definitions("opencode", false, false, Some(false)).is_empty());

    let unknown = definitions_for_model("opencode", None);
    assert!(!unknown.iter().any(|tool| tool.name == "ask_user"));
    assert!(unknown.iter().any(|tool| tool.name == "read_file"));
    let unknown_context = context_usage_definitions("opencode", false, false, None);
    assert!(!unknown_context.iter().any(|(name, _)| name == "ask_user"));

    assert!(definitions_for_request("gemini", Some(false)).is_empty());
    assert!(
        definitions_for_request("gemini", Some(true))
            .iter()
            .any(|tool| tool.name == "ask_user")
    );
    let unknown_mistral = definitions_for_request("mistral", None);
    assert!(!unknown_mistral.iter().any(|tool| tool.name == "ask_user"));
    assert!(unknown_mistral.iter().any(|tool| tool.name == "read_file"));
    let unknown_context = context_usage_definitions("mistral", false, false, None);
    assert!(!unknown_context.iter().any(|(name, _)| name == "ask_user"));
}

#[test]
fn copilot_messages_context_uses_anthropic_tool_definitions() {
    let definitions = context_usage_definitions("copilot", false, true, Some(true));
    let (_, definition) = definitions
        .iter()
        .find(|(name, _)| name == "read_file")
        .expect("read_file definition");

    assert!(definition.get("input_schema").is_some());
    assert!(definition.get("function").is_none());
}
