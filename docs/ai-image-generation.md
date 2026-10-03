# AI image generation

The shared `generate_image` tool is available on ChatGPT OAuth and `chatgpt_api` routes. Image input support is a separate capability: a text-only chat model can call the app's image-generation tool, while attachment validation still follows the selected model's image-input support. The same tool result and assistant attachment flow is used for both authentication methods:

- **ChatGPT OAuth:** the native service sends the request with the active ChatGPT account and workspace credentials. The request follows the image-generation implementation in the open-source Codex client.
- **OpenAI API key:** the `chatgpt_api` route sends the request to the documented OpenAI Images API with the connected API key. Up to three outputs are supported to match OpenChat's per-message attachment limit. `xhigh` and `max` quality require a GPT Image 2.5 Sunburst or Flare model; the default `gpt-image-2` accepts `low`, `medium`, `high`, and `auto`.

Generated PNG, JPEG, or WebP files are size- and signature-checked, stored under the local attachment directory for the conversation, and represented in chat history by attachment metadata. Image bytes are not included in tool activity output or SQLite message content. A Responses API continuation can attach the generated file to the next model turn without persisting a data URL.

The tool accepts a prompt and optional image model, dimensions, quality, and background. API-key generation can request multiple outputs. OAuth uses the Codex image backend contract and currently generates one output because that request path does not expose the public API's `n` field. Generation observes the active chat cancellation signal, including a cancellation received after the provider response and before local persistence. Provider, validation, and storage failures remain structured tool errors, and a failed attachment save removes its newly written files where possible.

## Authentication boundary

OpenAI's public image-generation contract is documented at the [Image Generation guide](https://developers.openai.com/api/docs/guides/image-generation) and uses `POST https://api.openai.com/v1/images/generations` for the API-key route.

The Codex repository currently implements OAuth image generation through `https://chatgpt.com/backend-api/codex/images/generations`; see the [Codex image backend](https://github.com/openai/codex/blob/main/codex-rs/ext/image-generation/src/backend.rs) and [Codex Images client](https://github.com/openai/codex/blob/main/codex-rs/codex-api/src/endpoint/images.rs). This ChatGPT backend route is private and is not a stable public API contract for third-party clients. Its availability, request shape, and OAuth access can change without notice. OpenChat keeps OAuth credentials inside the local Rust service, uses the Codex account/workspace request headers, and reports provider rejection rather than switching to the API-key endpoint or exposing credentials. The API-key route remains the documented public alternative.

OAuth compatibility has not been verified against a live account as part of automated tests. Tests use local mock HTTP responses; a successful build does not establish that a private provider endpoint will remain available.
