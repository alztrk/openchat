# Tauri IPC boundary

## Scope

This document separates renderer-to-Tauri invoke transport from the renderer-to-local-service NDJSON RPC transport. A limit on a service command or field does not establish a limit on the Tauri invoke envelope.

## Renderer to Tauri

The versions currently pinned in `Cargo.lock` are Tauri 2.12.4, `tauri-runtime` 2.12.2, `tauri-runtime-wry` 2.12.2, and Wry 0.57.0.

On Windows, Wry registers WebView2's `WebMessageReceived` handler, reads the posted string using `TryGetWebMessageAsString`, and passes the complete body as `Request<String>` to Tauri. Tauri's `handle_ipc_message` deserializes that body into the invoke message before dispatching the command. The inspected application and pinned Tauri/Wry path contains no application-configured maximum invoke-envelope byte count or aggregate in-flight byte budget.

This source trace does not establish the maximum message accepted by WebView2, the largest message that remains reliable, or the resource cost of large messages. The WebView2 API documentation describes JSON conversion and asynchronous delivery but does not state a maximum size in the referenced API contract. Do not infer an unbounded transport or a numeric limit from that omission.

Wry documents a 2 MiB limit for the Windows `with_html` builder input. That limit applies to initial HTML supplied to a WebView; it is not documented as a `postMessage` or Tauri invoke limit.

## Renderer to local service

The service process enforces separate protocol limits:

- Request frames are limited to 24 MiB each in `services/openchat-local-service/src/bootstrap/service.rs`.
- Aggregate in-flight request bytes are limited to 64 MiB, with at most 64 active operations in that service loop.
- Response frames are limited to 48 MiB in `services/openchat-local-service/src/transport/protocol.rs`.
- Individual RPC fields, attachments, and paginated responses have additional command-specific limits.

These limits apply after the frontend has invoked the host command that communicates with the service. They do not protect WebView2 or Tauri while receiving and deserializing a large invoke envelope.

## Verification status

The source-level path and distinctions above were checked against the dependency versions in `Cargo.lock` and the local-service source. A Windows WebView2 runtime test that progressively sends messages of controlled sizes and records delivery, failure, and process memory has not been run. Therefore no measured WebView2/Tauri invoke maximum or resource threshold is claimed here.

Before changing the Tauri/Wry boundary, measure the packaged Windows runtime with increasing payload sizes, record exact runtime versions and memory behavior, and verify both JSON and raw invoke paths. If a practical application cap is needed, enforce it before JSON serialization in the frontend and before deserialization/dispatch in the host, and document it as OpenChat's cap rather than a WebView2 platform limit.
