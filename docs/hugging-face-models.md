# Hugging Face model catalog

The **Models** page searches the public Hugging Face Hub and downloads compatible model files into OpenChat's managed model folders. Search metadata comes from the Hub model API; repository files, sizes, and the model card are loaded only after a model is selected. The selected repository is highlighted in the result list.

Search results are sorted by downloads, likes, or most recently updated. The page uses the opaque cursor returned by Hugging Face's `Link` response header for next/previous page navigation. A new query, format, or sort starts again from the first page.

The repository's root `README.md` is fetched at the same commit as its file list and rendered as Markdown. README contents are limited to 512 KiB and sanitized before rendering; HTML attributes and embedded active content are removed. Missing, restricted, oversized, and temporarily unavailable model cards have separate display states and do not prevent compatible files from being inspected or downloaded.

The file inventory separates auxiliary model artifacts from the main model files. Filename and directory markers classify vision files (`vision`, `mmproj`, or `projector`), MTP files, and other known components (`draft`, `speculator`, `medusa`, `adapter`, or `lora`). Matching weight files in GGUF, Safetensors, PyTorch, and ONNX formats are included in the inventory even when they are not part of the selected download group. Each listed component has its own download action and is added to the package folder selected by the main model option. Existing complete files are reused; conflicting files are never overwritten. Component downloads are optional and do not claim runtime support.

## Supported downloads

| Page option | Hub selection | Files downloaded | OpenChat folder |
| --- | --- | --- | --- |
| GGUF · llama.cpp | Models tagged for GGUF | One selected `.gguf` file, or every shard in a complete shard set | `models/llama` |
| Transformers · vLLM | Text-generation model repositories | Root model weights in Safetensors or PyTorch format, the weight index when present, and supported config/tokenizer files | `models/vllm` |
| ExLlama · EXL3 | Repositories carrying the Hub `exl3` tag | Safetensors weights and supported config/tokenizer files | `models/exllama` |

The ExLlama option downloads an already prepared EXL3 model snapshot. OpenChat does not convert ordinary Transformers weights to EXL3. Incomplete GGUF shard sets and repositories without a compatible weight/configuration set are not offered for download.

Downloads are pinned to the repository commit selected from Hub metadata. Files are streamed to the app data directory, report progress through the local service, and use HTTP range requests to continue partial files after a retry. The completed file or directory is registered through the same local-model catalog used by Settings.

## Access and runtime status

This integration does not send Hugging Face credentials. Public repositories can be inspected and downloaded. Private or gated repositories require Hub access and cannot be downloaded until OpenChat adds Hugging Face account/token support.

Downloading a model and being able to run it are separate states. The local model catalog reports whether the corresponding engine is installed and supported on the current device. A registered model can therefore exist while its runtime is unavailable.

## Hub endpoints

- Search and sorting: `GET https://huggingface.co/api/models` with format filter, sort, search, and opaque cursor query parameters
- Repository metadata and file sizes: `GET https://huggingface.co/api/models/{owner}/{repo}?blobs=true`
- Pinned model card: `GET https://huggingface.co/{owner}/{repo}/resolve/{commit}/README.md`
- Pinned file download: `GET https://huggingface.co/{owner}/{repo}/resolve/{commit}/{path}`

See the [Hugging Face Hub API](https://huggingface.co/docs/hub/en/api), [Hub downloads guide](https://huggingface.co/docs/huggingface_hub/main/en/guides/download), [GGUF documentation](https://huggingface.co/docs/hub/gguf), and [ExLlamaV3 conversion guide](https://github.com/turboderp-org/exllamav3/blob/master/doc/convert.md) for upstream format behavior.
