//! Provider-callable tools that translate design requests into typed canvas operations.

use serde_json::{Value, json};

use crate::{
    protocol::ServiceError,
    provider_schema::{ToolCall, ToolDefinition},
};

const MAXIMUM_DESIGN_TOOL_CALLS: usize = 400;
const MAXIMUM_DESIGN_NODES_PER_CALL: usize = 12;
pub(crate) const MAX_DESIGN_TOOL_ARGUMENT_BYTES: usize = 64 * 1024;

pub(crate) fn is_design_tool_name(name: &str) -> bool {
    matches!(
        name,
        "design_create_nodes"
            | "design_update_nodes"
            | "design_delete_nodes"
            | "design_update_document"
            | "design_add_reference_image"
            | "design_inspect_frame"
    )
}

/// Returns only the function tools available to temporary Design generation requests.
pub(crate) fn definitions(allow_frame_inspection: bool) -> Vec<ToolDefinition> {
    let mut definitions = vec![
        tool(
            "design_create_nodes",
            "Create up to 12 editable canvas nodes per call. Keep each call's arguments compact and make repeated calls for larger designs. Create parent frames before child nodes and set each child's parentId to its parent frame ID.",
            json!({
                "type": "object",
                "properties": { "nodes": { "type": "array", "items": node_schema(), "minItems": 1, "maxItems": MAXIMUM_DESIGN_NODES_PER_CALL } },
                "required": ["nodes"], "additionalProperties": false
            }),
        ),
        tool(
            "design_update_nodes",
            "Update up to 12 existing visible canvas nodes per call. Keep each call's arguments compact and make repeated calls for larger edits. Provide each node ID and only the fields to change; omitted fields remain unchanged.",
            json!({
                "type": "object",
                "properties": { "nodes": { "type": "array", "items": update_node_schema(), "minItems": 1, "maxItems": MAXIMUM_DESIGN_NODES_PER_CALL } },
                "required": ["nodes"], "additionalProperties": false
            }),
        ),
        tool(
            "design_delete_nodes",
            "Delete existing visible canvas nodes by ID. Deleting a parent also deletes its descendants.",
            json!({
                "type": "object",
                "properties": { "nodeIds": { "type": "array", "items": { "type": "string", "minLength": 1, "maxLength": 128 }, "minItems": 1, "maxItems": MAXIMUM_DESIGN_NODES_PER_CALL } },
                "required": ["nodeIds"], "additionalProperties": false
            }),
        ),
        tool(
            "design_update_document",
            "Update the active Design document's metadata or structured data. Only include fields that change. Supported fields are name, pages, activePageId, variables, textStyles, comments, prototypeFlows, prototypeInteractions, motionTracks, and components. IDs and references must point to items in this document.",
            json!({
                "type": "object",
                "properties": { "patch": document_patch_schema() },
                "required": ["patch"], "additionalProperties": false
            }),
        ),
        tool(
            "design_add_reference_image",
            "Place one of the user-provided image references into the active design as an editable image node.",
            json!({
                "type": "object",
                "properties": {
                    "referenceId": { "type": "string", "minLength": 1, "maxLength": 128 },
                    "node": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string", "minLength": 1, "maxLength": 128 },
                            "name": { "type": "string", "minLength": 1, "maxLength": 256 },
                            "x": { "type": "number" }, "y": { "type": "number" },
                            "width": { "type": "number", "exclusiveMinimum": 0 }, "height": { "type": "number", "exclusiveMinimum": 0 },
                            "parentId": { "type": ["string", "null"] }, "rotation": { "type": "number" },
                            "visible": { "type": "boolean" }, "locked": { "type": "boolean" },
                            "fit": { "type": "string", "enum": ["cover", "contain"] }, "altText": { "type": "string", "maxLength": 2000 }
                        },
                        "required": ["id", "name", "x", "y", "width", "height", "parentId", "rotation", "visible", "locked", "fit", "altText"],
                        "additionalProperties": false
                    }
                },
                "required": ["referenceId", "node"], "additionalProperties": false
            }),
        ),
    ];
    if allow_frame_inspection {
        definitions.push(tool(
            "design_inspect_frame",
            "Capture a rendered screenshot of an existing or newly created visible frame for visual review.",
            json!({
                "type": "object",
                "properties": { "nodeId": { "type": "string", "minLength": 1, "maxLength": 128 } },
                "required": ["nodeId"], "additionalProperties": false
            }),
        ));
    }
    definitions
}

/// Converts provider tool calls into the canvas operation envelope consumed by the renderer.
pub(crate) fn calls_to_operations(
    calls: &[ToolCall],
    allow_frame_inspection: bool,
) -> Result<String, ServiceError> {
    if calls.is_empty() || calls.len() > MAXIMUM_DESIGN_TOOL_CALLS {
        return Err(invalid_design_tool_call());
    }
    let mut operations = Vec::new();
    for call in calls {
        let arguments = call
            .arguments
            .as_object()
            .ok_or_else(invalid_design_tool_call)?;
        match call.name.as_str() {
            "design_create_nodes" => {
                let nodes = read_bounded_array(arguments.get("nodes"))?;
                operations.extend(
                    nodes
                        .into_iter()
                        .map(|node| json!({ "type": "create", "node": node })),
                );
            }
            "design_update_nodes" => {
                let nodes = read_bounded_array(arguments.get("nodes"))?;
                operations.extend(
                    nodes
                        .into_iter()
                        .map(|node| json!({ "type": "update", "node": node })),
                );
            }
            "design_delete_nodes" => {
                let node_ids = read_bounded_array(arguments.get("nodeIds"))?;
                for node_id in node_ids {
                    if !node_id
                        .as_str()
                        .is_some_and(|id| !id.is_empty() && id.len() <= 128)
                    {
                        return Err(invalid_design_tool_call());
                    }
                    operations.push(json!({ "type": "delete", "nodeId": node_id }));
                }
            }
            "design_update_document" => {
                let patch = arguments
                    .get("patch")
                    .filter(|value| value.is_object())
                    .ok_or_else(invalid_design_tool_call)?;
                operations.push(json!({ "type": "document", "patch": patch }));
            }
            "design_add_reference_image" => {
                let reference_id = arguments
                    .get("referenceId")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty() && id.len() <= 128)
                    .ok_or_else(invalid_design_tool_call)?;
                let node = arguments
                    .get("node")
                    .filter(|value| value.is_object())
                    .ok_or_else(invalid_design_tool_call)?;
                operations.push(json!({ "type": "create-image-from-reference", "referenceId": reference_id, "node": node }));
            }
            "design_inspect_frame" => {
                if !allow_frame_inspection {
                    return Err(invalid_design_tool_call());
                }
                let node_id = arguments
                    .get("nodeId")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty() && id.len() <= 128)
                    .ok_or_else(invalid_design_tool_call)?;
                operations.push(json!({ "type": "inspect-frame", "nodeId": node_id }));
            }
            _ => return Err(invalid_design_tool_call()),
        }
        if operations.len() > MAXIMUM_DESIGN_TOOL_CALLS {
            return Err(invalid_design_tool_call());
        }
    }
    serde_json::to_string(&json!({ "operations": operations }))
        .map_err(|_| invalid_design_tool_call())
}

fn tool(name: &str, description: &str, parameters: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.to_owned(),
        description: description.to_owned(),
        parameters,
    }
}

fn node_schema() -> Value {
    let mut properties = serde_json::Map::new();
    if let Value::Object(base_properties) = json!({
        "id": { "type": "string", "minLength": 1, "maxLength": 128 },
        "type": { "type": "string", "enum": ["frame", "rectangle", "ellipse", "path", "text", "image", "imported-svg"] },
        "name": { "type": "string", "minLength": 1, "maxLength": 256 }, "pageId": { "type": "string" },
        "x": { "type": "number" }, "y": { "type": "number" },
        "width": { "type": "number", "exclusiveMinimum": 0 }, "height": { "type": "number", "exclusiveMinimum": 0 },
        "parentId": { "type": ["string", "null"] }, "rotation": { "type": "number" },
        "visible": { "type": "boolean" }, "locked": { "type": "boolean" },
        "fill": { "type": "string", "description": "Solid fill color as #RRGGBB or a supported transparent color." },
        "fillStyle": fill_style_schema(), "fillVariableId": { "type": "string" },
        "stroke": { "type": "string" }, "strokeWidth": { "type": "number", "minimum": 0 },
        "radius": { "type": "number", "minimum": 0 }, "cornerRadii": corner_radii_schema(),
        "scrollable": { "type": "boolean" }, "autoLayout": auto_layout_schema(),
        "points": path_points_schema(), "closed": { "type": "boolean" },
        "content": { "type": "string", "maxLength": 4000 }, "color": { "type": "string" },
        "fontSize": { "type": "number", "exclusiveMinimum": 0 }, "fontWeight": { "type": "integer", "minimum": 100, "maximum": 900 },
        "fontFamily": { "type": "string", "enum": ["sans-serif", "serif", "monospace"] },
        "lineHeight": { "type": "number", "exclusiveMinimum": 0 }, "letterSpacing": { "type": "number" },
        "textAlign": { "type": "string", "enum": ["left", "center", "right"] },
        "colorVariableId": { "type": "string" }, "textStyleId": { "type": "string" }
    }) {
        properties.extend(base_properties);
    }
    if let Value::Object(additional_properties) = json!({
        "effects": effects_schema(), "constraints": constraints_schema(),
        "responsiveOverrides": responsive_overrides_schema(),
        "svg": { "type": "string", "maxLength": 32768 }, "tokens": canvas_tokens_schema(),
        "assetId": { "type": "string" }, "mimeType": { "type": "string", "enum": ["image/png", "image/jpeg", "image/webp"] },
        "sizeBytes": { "type": "integer", "minimum": 1 }, "fit": { "type": "string", "enum": ["cover", "contain"] },
        "altText": { "type": "string", "maxLength": 2000 }, "componentRef": component_reference_schema()
    }) {
        properties.extend(additional_properties);
    }
    json!({
        "type": "object",
        "description": "One editable Design canvas node. Coordinates are in canvas units. Children use their parent frame's coordinate space. Supply fields required by the selected type.",
        "properties": properties,
        "required": ["id", "type", "name", "x", "y", "width", "height", "parentId", "rotation", "visible", "locked"],
        "additionalProperties": false
    })
}

fn effects_schema() -> Value {
    json!({
        "type": "array", "items": { "type": "object", "properties": {
            "type": { "type": "string", "enum": ["shadow"] }, "color": { "type": "string" },
            "offsetX": { "type": "number" }, "offsetY": { "type": "number" }, "blur": { "type": "number", "minimum": 0 }
        }, "required": ["type", "color", "offsetX", "offsetY", "blur"], "additionalProperties": false }
    })
}

fn constraints_schema() -> Value {
    json!({
        "type": "object", "properties": {
            "horizontal": { "type": "string", "enum": ["start", "end", "start-end", "center", "scale"] },
            "vertical": { "type": "string", "enum": ["start", "end", "start-end", "center", "scale"] }
        }, "required": ["horizontal", "vertical"], "additionalProperties": false
    })
}

fn responsive_overrides_schema() -> Value {
    json!({
        "type": "object", "properties": {
            "mobile": responsive_geometry_schema(), "tablet": responsive_geometry_schema()
        }, "additionalProperties": false
    })
}

fn corner_radii_schema() -> Value {
    json!({
        "type": "object", "properties": {
            "topLeft": { "type": "number", "minimum": 0 }, "topRight": { "type": "number", "minimum": 0 },
            "bottomRight": { "type": "number", "minimum": 0 }, "bottomLeft": { "type": "number", "minimum": 0 }
        }, "required": ["topLeft", "topRight", "bottomRight", "bottomLeft"], "additionalProperties": false
    })
}

fn auto_layout_schema() -> Value {
    json!({
        "type": "object", "properties": {
            "direction": { "type": "string", "enum": ["horizontal", "vertical"] },
            "alignment": { "type": "string", "enum": ["start", "center", "end", "stretch"] },
            "gap": { "type": "number", "minimum": 0 }, "padding": { "type": "number", "minimum": 0 }
        }, "required": ["direction", "alignment", "gap", "padding"], "additionalProperties": false
    })
}

fn path_points_schema() -> Value {
    json!({
        "type": "array", "items": { "type": "object", "properties": {
            "x": { "type": "number" }, "y": { "type": "number" }
        }, "required": ["x", "y"], "additionalProperties": false }, "minItems": 2, "maxItems": 1000
    })
}

fn canvas_tokens_schema() -> Value {
    json!({
        "type": "object", "properties": {
            "background": { "type": "string" }, "surface": { "type": "string" }, "text": { "type": "string" },
            "primary": { "type": "string" }, "accent": { "type": "string" }, "border": { "type": "string" },
            "fontFamily": { "type": "string", "enum": ["Source Sans 3", "Arial", "Georgia", "monospace"] },
            "typeScale": { "type": "integer", "enum": [14, 16, 18] }, "spacingUnit": { "type": "integer", "enum": [4, 8, 12] }
        }, "required": ["background", "surface", "text", "primary", "accent", "border", "fontFamily", "typeScale", "spacingUnit"], "additionalProperties": false
    })
}

fn component_reference_schema() -> Value {
    json!({
        "type": "object", "properties": {
            "componentId": { "type": "string" }, "sourceNodeId": { "type": "string" }, "variantId": { "type": "string" },
            "role": { "type": "string", "enum": ["master", "variant", "instance"] }
        }, "required": ["componentId", "sourceNodeId", "variantId", "role"], "additionalProperties": false
    })
}

fn fill_style_schema() -> Value {
    json!({
        "oneOf": [
            { "type": "object", "properties": {
                "type": { "type": "string", "enum": ["gradient"] },
                "endColor": { "type": "string" }, "angle": { "type": "number", "minimum": 0, "maximum": 360 },
                "stops": { "type": "array", "items": { "type": "object", "properties": {
                    "position": { "type": "number", "minimum": 0, "maximum": 1 }, "color": { "type": "string" }
                }, "required": ["position", "color"], "additionalProperties": false }, "minItems": 2, "maxItems": 16 }
            }, "required": ["type", "endColor", "angle"], "additionalProperties": false },
            { "type": "object", "properties": {
                "type": { "type": "string", "enum": ["image"] }, "assetId": { "type": "string" },
                "mimeType": { "type": "string", "enum": ["image/png", "image/jpeg", "image/webp"] },
                "sizeBytes": { "type": "integer", "minimum": 1 }, "fit": { "type": "string", "enum": ["cover", "contain"] }
            }, "required": ["type", "assetId", "mimeType", "sizeBytes", "fit"], "additionalProperties": false }
        ]
    })
}

fn responsive_geometry_schema() -> Value {
    json!({
        "type": "object", "properties": {
            "x": { "type": "number" }, "y": { "type": "number" },
            "width": { "type": "number", "exclusiveMinimum": 0 }, "height": { "type": "number", "exclusiveMinimum": 0 }
        }, "required": ["x", "y", "width", "height"], "additionalProperties": false
    })
}

fn document_patch_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "minLength": 1, "maxLength": 128 },
            "pages": { "type": "array", "items": { "type": "object", "properties": {
                "id": { "type": "string", "minLength": 1, "maxLength": 128 }, "name": { "type": "string", "minLength": 1, "maxLength": 128 }
            }, "required": ["id", "name"], "additionalProperties": false }, "minItems": 1, "maxItems": 100 },
            "activePageId": { "type": "string", "minLength": 1, "maxLength": 128 },
            "variables": { "type": "array", "items": { "type": "object", "properties": {
                "id": { "type": "string" }, "name": { "type": "string", "minLength": 1, "maxLength": 128 }, "value": { "type": "string" }
            }, "required": ["id", "name", "value"], "additionalProperties": false }, "maxItems": 500 },
            "textStyles": { "type": "array", "items": { "type": "object", "properties": {
                "id": { "type": "string" }, "name": { "type": "string", "minLength": 1, "maxLength": 128 }, "color": { "type": "string" },
                "fontSize": { "type": "number", "exclusiveMinimum": 0 }, "fontWeight": { "type": "integer", "minimum": 100, "maximum": 900 },
                "fontFamily": { "type": "string", "enum": ["sans-serif", "serif", "monospace"] }, "lineHeight": { "type": "number", "exclusiveMinimum": 0 },
                "letterSpacing": { "type": "number" }, "textAlign": { "type": "string", "enum": ["left", "center", "right"] }
            }, "required": ["id", "name", "color", "fontSize", "fontWeight"], "additionalProperties": false }, "maxItems": 2000 },
            "comments": { "type": "array", "items": { "type": "object", "properties": {
                "id": { "type": "string" }, "pageId": { "type": "string" }, "nodeId": { "type": ["string", "null"] },
                "text": { "type": "string", "maxLength": 4000 }, "point": { "type": ["object", "null"], "properties": { "x": { "type": "number" }, "y": { "type": "number" } }, "required": ["x", "y"], "additionalProperties": false },
                "createdAt": { "type": "integer", "minimum": 0 }, "resolved": { "type": "boolean" }
            }, "required": ["id", "pageId", "nodeId", "text", "point", "createdAt", "resolved"], "additionalProperties": false }, "maxItems": 10000 },
            "prototypeFlows": { "type": "array", "items": { "type": "object", "properties": {
                "id": { "type": "string" }, "name": { "type": "string", "minLength": 1, "maxLength": 128 },
                "pageId": { "type": "string" }, "startNodeId": { "type": "string" }
            }, "required": ["id", "name", "pageId", "startNodeId"], "additionalProperties": false }, "maxItems": 1000 },
            "prototypeInteractions": { "type": "array", "items": { "type": "object", "properties": {
                "id": { "type": "string" }, "sourceNodeId": { "type": "string" }, "targetNodeId": { "type": "string" },
                "trigger": { "type": "string", "enum": ["click", "hover"] }, "presentation": { "type": "string", "enum": ["navigate", "overlay"] },
                "transition": { "type": "string", "enum": ["instant", "dissolve", "slide"] }, "durationMs": { "type": "integer", "minimum": 0, "maximum": 10000 }
            }, "required": ["id", "sourceNodeId", "targetNodeId", "trigger", "presentation", "transition", "durationMs"], "additionalProperties": false }, "maxItems": 10000 },
            "motionTracks": { "type": "array", "items": { "type": "object", "properties": {
                "id": { "type": "string" }, "nodeId": { "type": "string" }, "property": { "type": "string", "enum": ["x", "y", "rotation", "opacity", "scale"] },
                "keyframes": { "type": "array", "items": { "type": "object", "properties": {
                    "id": { "type": "string" }, "timeMs": { "type": "integer", "minimum": 0 }, "value": { "type": "number" },
                    "easing": { "type": "string", "enum": ["linear", "ease-in", "ease-out", "ease-in-out"] }
                }, "required": ["id", "timeMs", "value", "easing"], "additionalProperties": false }, "minItems": 1, "maxItems": 1000 }
            }, "required": ["id", "nodeId", "property", "keyframes"], "additionalProperties": false }, "maxItems": 5000 },
            "components": { "type": "array", "items": { "type": "object", "properties": {
                "id": { "type": "string" }, "name": { "type": "string", "minLength": 1, "maxLength": 128 },
                "variants": { "type": "array", "items": { "type": "object", "properties": {
                    "id": { "type": "string" }, "name": { "type": "string", "minLength": 1, "maxLength": 128 }, "rootNodeId": { "type": "string" }
                }, "required": ["id", "name", "rootNodeId"], "additionalProperties": false }, "minItems": 1, "maxItems": 100 }
            }, "required": ["id", "name", "variants"], "additionalProperties": false }, "maxItems": 1000 }
        },
        "additionalProperties": false
    })
}

fn update_node_schema() -> Value {
    let mut schema = node_schema();
    schema["required"] = json!(["id"]);
    schema
}

fn read_bounded_array(value: Option<&Value>) -> Result<Vec<Value>, ServiceError> {
    value
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= MAXIMUM_DESIGN_NODES_PER_CALL)
        .cloned()
        .ok_or_else(invalid_design_tool_call)
}

fn invalid_design_tool_call() -> ServiceError {
    ServiceError::new(
        "invalid_design_tool_call",
        "The Design agent returned an invalid canvas tool call.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_tool_definitions_are_scoped_and_have_valid_update_contract() {
        let definitions = definitions(false);
        let names = definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            [
                "design_create_nodes",
                "design_update_nodes",
                "design_delete_nodes",
                "design_update_document",
                "design_add_reference_image",
            ]
        );
        let update = definitions
            .iter()
            .find(|definition| definition.name == "design_update_nodes")
            .expect("update tool is available");
        assert_eq!(
            definitions[0].parameters["properties"]["nodes"]["maxItems"],
            MAXIMUM_DESIGN_NODES_PER_CALL
        );
        assert_eq!(
            update.parameters["properties"]["nodes"]["items"]["required"],
            json!(["id"])
        );
        assert!(!names.contains(&"design_inspect_frame"));
        let create = &definitions[0].parameters["properties"]["nodes"]["items"]["properties"];
        assert!(create.get("fillStyle").is_some());
        assert!(create.get("cornerRadii").is_some());
        assert!(create.get("autoLayout").is_some());
        let document_patch = &definitions[3].parameters["properties"]["patch"]["properties"];
        for field in [
            "pages",
            "variables",
            "textStyles",
            "comments",
            "prototypeFlows",
            "prototypeInteractions",
            "motionTracks",
            "components",
        ] {
            assert!(
                document_patch.get(field).is_some(),
                "missing {field} schema"
            );
        }
    }

    #[test]
    fn frame_inspection_is_only_defined_when_authorized() {
        let definitions = definitions(true);
        assert!(
            definitions
                .iter()
                .any(|tool| tool.name == "design_inspect_frame")
        );
    }

    #[test]
    fn tool_calls_are_converted_to_typed_canvas_operations() {
        let calls = [
            ToolCall {
                id: "create-1".to_owned(),
                name: "design_create_nodes".to_owned(),
                arguments: json!({ "nodes": [{ "id": "frame-1", "type": "frame" }] }),
            },
            ToolCall {
                id: "update-1".to_owned(),
                name: "design_update_nodes".to_owned(),
                arguments: json!({ "nodes": [{ "id": "frame-1", "name": "Home" }] }),
            },
            ToolCall {
                id: "delete-1".to_owned(),
                name: "design_delete_nodes".to_owned(),
                arguments: json!({ "nodeIds": ["old-layer"] }),
            },
            ToolCall {
                id: "document-1".to_owned(),
                name: "design_update_document".to_owned(),
                arguments: json!({ "patch": { "name": "Product" } }),
            },
            ToolCall {
                id: "reference-1".to_owned(),
                name: "design_add_reference_image".to_owned(),
                arguments: json!({ "referenceId": "upload-1", "node": { "id": "image-1" } }),
            },
        ];

        let output = calls_to_operations(&calls, false).expect("valid tool calls convert");
        let output: Value = serde_json::from_str(&output).expect("operation envelope is JSON");
        assert_eq!(output["operations"].as_array().map(Vec::len), Some(5));
        assert_eq!(output["operations"][0]["type"], "create");
        assert_eq!(output["operations"][1]["type"], "update");
        assert_eq!(output["operations"][2]["type"], "delete");
        assert_eq!(output["operations"][3]["type"], "document");
        assert_eq!(
            output["operations"][4]["type"],
            "create-image-from-reference"
        );
    }

    #[test]
    fn frame_inspection_call_is_rejected_when_not_authorized() {
        let calls = [ToolCall {
            id: "inspect-1".to_owned(),
            name: "design_inspect_frame".to_owned(),
            arguments: json!({ "nodeId": "frame-1" }),
        }];

        assert!(calls_to_operations(&calls, false).is_err());
    }
}
