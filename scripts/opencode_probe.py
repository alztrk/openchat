#!/usr/bin/env python3
"""Probe OpenCode's documented model catalog and inference endpoints."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[1]
STATE_DIR = ROOT / ".opencode_probe"
CREDENTIALS_PATH = STATE_DIR / "credentials.json"
SESSION_PATH = STATE_DIR / "session-id"
CATALOG_URL = "https://models.opencode.ai/api.json"
CONSOLE_URL = "https://opencode.ai/console"
CLIENT_ID = "opencode-cli"
DEFAULT_UA = "OpenChat-opencode-probe/1.0"
TIMEOUT = 45


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


OPENER = urllib.request.build_opener(
    urllib.request.HTTPSHandler(), NoRedirect()
)


def user_agent() -> str:
    return os.environ.get("OPENCODE_PROBE_UA", DEFAULT_UA)


def request(
    url: str,
    method: str = "GET",
    headers: dict[str, str] | None = None,
    body: dict[str, Any] | None = None,
) -> tuple[int, Any, bytes]:
    request_headers = {"User-Agent": user_agent(), **(headers or {})}
    data = None
    if body is not None:
        data = json.dumps(body, ensure_ascii=False, separators=(",", ":")).encode()
        request_headers.setdefault("Content-Type", "application/json")
    req = urllib.request.Request(url, data=data, headers=request_headers, method=method)
    try:
        with OPENER.open(req, timeout=TIMEOUT) as response:
            return response.status, response.headers, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.headers, error.read()


def safe_headers(headers: Any) -> dict[str, str]:
    return {
        key: "[REDACTED]" if sensitive_name(key) else value
        for key, value in headers.items()
    }


def sensitive_name(name: str) -> bool:
    lowered = name.lower().replace("_", "-")
    return any(part in lowered for part in ("authorization", "cookie", "token", "secret", "credential", "api-key"))


def redact(value: Any) -> Any:
    if isinstance(value, dict):
        return {
            key: "[REDACTED]" if sensitive_name(key) else redact(item)
            for key, item in value.items()
        }
    if isinstance(value, list):
        return [redact(item) for item in value]
    if isinstance(value, str) and value.startswith(("https://", "http://")):
        return safe_url(value)
    return value


def safe_url(value: str) -> str:
    parsed = urllib.parse.urlsplit(value)
    host = parsed.hostname or ""
    if parsed.port is not None:
        host = f"{host}:{parsed.port}"
    return urllib.parse.urlunsplit((parsed.scheme, host, parsed.path, "", ""))


def response_text(data: bytes) -> str:
    return data.decode("utf-8", errors="replace")


def load_json(data: bytes, label: str) -> Any:
    try:
        return json.loads(data)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"{label} returned invalid JSON: {error}") from error


def save_artifact(directory: str | None, name: str, data: bytes) -> None:
    if directory is None:
        return
    target = pathlib.Path(directory).resolve()
    target.mkdir(parents=True, exist_ok=True)
    (target / name).write_bytes(data)


def fetch_catalog(save_dir: str | None = None) -> dict[str, Any]:
    status, headers, data = request(CATALOG_URL, headers={"Accept": "application/json"})
    print(f"GET {CATALOG_URL} -> HTTP {status}")
    print(f"Content-Type: {headers.get('Content-Type', 'unknown')}")
    save_artifact(save_dir, "catalog-response.json", data)
    if status != 200:
        raise RuntimeError(f"Model catalog request failed with HTTP {status}: {response_text(data)[:2000]}")
    parsed = load_json(data, "Model catalog")
    if not isinstance(parsed, dict):
        raise RuntimeError("Model catalog has an unsupported shape")
    return parsed


def free_models(catalog: dict[str, Any]) -> list[tuple[str, dict[str, Any], dict[str, Any]]]:
    provider = catalog.get("opencode")
    if not isinstance(provider, dict) or not isinstance(provider.get("models"), dict):
        raise RuntimeError("Catalog has no OpenCode provider model map")
    result = []
    for model_id, model in provider["models"].items():
        if not isinstance(model, dict):
            continue
        cost = model.get("cost")
        if not isinstance(cost, dict) or cost.get("input") != 0 or cost.get("output") != 0:
            continue
        tiers = cost.get("tiers", [])
        if isinstance(tiers, list) and any(
            not isinstance(tier, dict) or tier.get("input") != 0 or tier.get("output") != 0
            for tier in tiers
        ):
            continue
        result.append((model_id, provider, model))
    return result


def command_models(args: argparse.Namespace) -> None:
    catalog = fetch_catalog(args.save_dir)
    rows = free_models(catalog)
    provider = catalog["opencode"]
    print(f"Provider API: {provider.get('api', '[not set]')}")
    print(f"Provider package: {provider.get('npm', '[not set]')}")
    print(f"Free models (input and output cost are zero): {len(rows)}")
    for model_id, _provider, model in rows:
        cost = model.get("cost", {})
        print(
            f"- {model_id} | api={model.get('api', provider.get('api', '[provider default]'))} "
            f"| package={model.get('npm', provider.get('npm', '[provider default]'))} "
            f"| input={cost.get('input')} output={cost.get('output')}"
        )


def session_id() -> str:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    if SESSION_PATH.exists():
        return SESSION_PATH.read_text(encoding="utf-8").strip()
    value = f"openchat-probe-{uuid.uuid4()}"
    SESSION_PATH.write_text(value, encoding="utf-8")
    return value


def openai_endpoint(base_url: str, model: dict[str, Any]) -> str:
    package = model.get("npm")
    if package is None:
        package = "@ai-sdk/openai-compatible"
    if package == "@ai-sdk/openai":
        suffix = "/responses"
    elif package == "@ai-sdk/openai-compatible":
        suffix = "/chat/completions"
    else:
        raise RuntimeError(f"Unsupported OpenCode model package for this probe: {package}")
    parsed = urllib.parse.urlsplit(base_url)
    if parsed.scheme != "https" or not parsed.netloc or parsed.query or parsed.fragment:
        raise RuntimeError("Model API must be an HTTPS URL without query or fragment")
    return base_url.rstrip("/") + suffix


def affinity_headers(session: str, project_id: str | None = None) -> dict[str, str]:
    headers = {
        "Authorization": "Bearer public",
        "Accept": "text/event-stream",
        "Content-Type": "application/json",
        "x-opencode-session-id": session,
        "x-session-affinity": session,
        "X-Session-Id": session,
        "x-opencode-session": session,
        "x-opencode-client": "OpenChat",
    }
    if project_id:
        headers["x-opencode-project"] = project_id
    return headers


def redacted_request(url: str, headers: dict[str, str], body: dict[str, Any]) -> None:
    print(f"POST {url}")
    display_headers = {"User-Agent": user_agent(), **headers}
    for key, value in display_headers.items():
        if sensitive_name(key):
            if key.lower() == "authorization" and value == "Bearer public":
                value = "Bearer public"
            else:
                value = "[REDACTED]"
        print(f"{key}: {value}")
    print(json.dumps(redact(body), ensure_ascii=False, indent=2))


def stream_request(
    url: str,
    headers: dict[str, str],
    body: dict[str, Any],
    raw: bool,
    verbose: bool,
    save_dir: str | None,
) -> int:
    data = json.dumps(body, ensure_ascii=False, separators=(",", ":")).encode()
    if verbose:
        redacted_request(url, headers, body)
    req = urllib.request.Request(url, data=data, headers={"User-Agent": user_agent(), **headers}, method="POST")
    try:
        response = OPENER.open(req, timeout=TIMEOUT)
    except urllib.error.HTTPError as error:
        response = error
    print(f"HTTP {response.status}")
    print(f"Content-Type: {response.headers.get('Content-Type', 'unknown')}")
    save_artifact(
        save_dir,
        "inference-response-meta.json",
        json.dumps({"status": response.status, "headers": safe_headers(response.headers)}, indent=2).encode(),
    )
    if verbose:
        print(json.dumps(safe_headers(response.headers), indent=2))
    output = bytearray()
    is_event_stream = "text/event-stream" in response.headers.get("Content-Type", "").lower()
    if not 200 <= response.status < 300 or not is_event_stream:
        output.extend(response.read())
        text = response_text(bytes(output))
        print(text if raw or response.status >= 400 else text[:8000])
        save_artifact(save_dir, "inference-response.body", bytes(output))
        save_request_artifact(save_dir, url, headers, body)
        code = response.status
        response.close()
        return code
    try:
        while True:
            line = response.readline()
            if not line:
                break
            output.extend(line)
            text = response_text(line).rstrip("\r\n")
            if raw:
                print(text)
            elif text.startswith("event:"):
                print(text)
            elif text.startswith("data:"):
                value = text[5:].strip()
                if value == "[DONE]":
                    print("data: [DONE]")
                    continue
                try:
                    parsed = json.loads(value)
                    if isinstance(parsed, dict):
                        print("data: " + json.dumps(parsed, ensure_ascii=False)[:1200])
                    else:
                        print("data: " + json.dumps(parsed, ensure_ascii=False)[:1200])
                except json.JSONDecodeError:
                    print(text[:1200])
            elif text == "" and raw:
                print()
    finally:
        response.close()
    save_artifact(save_dir, "inference-response.sse", bytes(output))
    save_request_artifact(save_dir, url, headers, body)
    return 200


def save_request_artifact(
    directory: str | None, url: str, headers: dict[str, str], body: dict[str, Any]
) -> None:
    if directory is None:
        return
    artifact = {
        "method": "POST",
        "url": url,
        "headers": {
            "User-Agent": user_agent(),
            **{
                key: "Bearer public" if key.lower() == "authorization" and value == "Bearer public"
                else "[REDACTED]" if sensitive_name(key) else value
                for key, value in headers.items()
            },
        },
        "body": redact(body),
    }
    save_artifact(directory, "inference-request.json", json.dumps(artifact, indent=2).encode())


def request_body(package: str, model_id: str, prompt: str, session: str) -> dict[str, Any]:
    if package == "@ai-sdk/openai":
        return {
            "model": model_id,
            "input": prompt,
            "stream": True,
            "prompt_cache_key": session,
        }
    if package == "@ai-sdk/openai-compatible":
        return {
            "model": model_id,
            "messages": [{"role": "user", "content": prompt}],
            "stream": True,
            "prompt_cache_key": session,
        }
    raise RuntimeError(f"Unsupported model package for this probe: {package}")


def command_anon_chat(args: argparse.Namespace) -> None:
    catalog = fetch_catalog(args.save_dir)
    matches = free_models(catalog)
    selected = next((entry for entry in matches if entry[0] == args.model), None)
    if selected is None:
        raise RuntimeError(f"Model is not listed as free in the current catalog: {args.model}")
    model_id, provider, model = selected
    base_url = model.get("api") or provider.get("api")
    if not isinstance(base_url, str):
        raise RuntimeError("Model catalog does not provide an inference URL")
    package = model.get("npm") or provider.get("npm") or "@ai-sdk/openai-compatible"
    url = openai_endpoint(base_url, {"npm": package})
    session = args.session_id or session_id()
    body = request_body(package, model_id, args.prompt, session)
    headers = affinity_headers(session, args.project_id)
    if args.dry_run:
        redacted_request(url, headers, body)
        save_request_artifact(args.save_dir, url, headers, body)
        return
    status = stream_request(url, headers, body, args.raw, args.verbose, args.save_dir)
    if not 200 <= status < 300:
        raise RuntimeError("Inference returned a non-success HTTP status; no header or credential variations were attempted")


def credentials() -> dict[str, Any]:
    if not CREDENTIALS_PATH.is_file():
        raise RuntimeError("No Console credential saved. Run the login command after reviewing its authorization prompt.")
    data = json.loads(CREDENTIALS_PATH.read_text(encoding="utf-8"))
    if not isinstance(data, dict) or not isinstance(data.get("access_token"), str):
        raise RuntimeError("Saved Console credential file has an invalid shape")
    return data


def write_credentials(value: dict[str, Any]) -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(value, indent=2).encode("utf-8")
    descriptor = os.open(CREDENTIALS_PATH, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "wb") as credential_file:
        credential_file.write(payload)
    os.chmod(CREDENTIALS_PATH, 0o600)


def post_json(url: str, values: dict[str, Any]) -> tuple[int, Any, bytes]:
    encoded = json.dumps(values, separators=(",", ":")).encode()
    req = urllib.request.Request(
        url,
        data=encoded,
        headers={"User-Agent": user_agent(), "Content-Type": "application/json", "Accept": "application/json"},
        method="POST",
    )
    try:
        with OPENER.open(req, timeout=TIMEOUT) as response:
            return response.status, response.headers, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.headers, error.read()


def command_login(_args: argparse.Namespace) -> None:
    server = os.environ.get("OPENCODE_CONSOLE_URL", CONSOLE_URL).rstrip("/")
    status, _headers, data = post_json(
        server + "/auth/device/code",
        {"client_id": CLIENT_ID, "supports_org_scope": "true"},
    )
    if status != 200:
        raise RuntimeError(f"Device authorization request failed with HTTP {status}: {response_text(data)[:1000]}")
    device = load_json(data, "Device authorization")
    print(f"Verification URL: {device['verification_uri_complete']}")
    print(f"User code: {device['user_code']}")
    input("After approving access in the browser, press Enter to continue polling: ")
    deadline = time.monotonic() + int(device["expires_in"])
    interval = max(1, int(device["interval"]))
    while time.monotonic() < deadline:
        time.sleep(interval)
        status, _headers, data = post_json(
            server + "/auth/device/token",
            {
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
                "device_code": device["device_code"],
                "client_id": CLIENT_ID,
            },
        )
        token = load_json(data, "Device token")
        if status == 200 and isinstance(token.get("access_token"), str):
            auth_headers = {"Authorization": "Bearer " + token["access_token"], "Accept": "application/json"}
            user_status, _user_headers, user_data = request(server + "/api/user", headers=auth_headers)
            org_status, _org_headers, org_data = request(server + "/api/orgs", headers=auth_headers)
            if user_status != 200 or org_status != 200:
                raise RuntimeError("Console identity lookup failed after device authorization")
            user = load_json(user_data, "Console user")
            orgs = load_json(org_data, "Console organizations")
            org_id = token.get("org_id")
            if org_id is None and isinstance(orgs, list) and len(orgs) == 1:
                org_id = orgs[0].get("id")
            save = {
                "server": server,
                "access_token": token["access_token"],
                "refresh_token": token["refresh_token"],
                "expires_at": int(time.time()) + int(token["expires_in"]),
                "org_id": org_id,
                "account_id": user.get("id") if isinstance(user, dict) else None,
                "client_id": CLIENT_ID,
            }
            try:
                write_credentials(save)
            except OSError as error:
                raise RuntimeError(f"Could not save Console credentials securely: {error}") from error
            print(f"Console credential saved to {CREDENTIALS_PATH}; token values were not printed.")
            return
        error = token.get("error") if isinstance(token, dict) else None
        if error == "authorization_pending":
            continue
        if error == "slow_down":
            interval += 5
            continue
        raise RuntimeError(f"Device authorization failed (HTTP {status}, error={error or 'unknown'})")
    raise RuntimeError("Device authorization expired")


def refresh_credentials(saved: dict[str, Any]) -> dict[str, Any]:
    status, _headers, data = post_json(
        saved["server"].rstrip("/") + "/auth/device/token",
        {
            "grant_type": "refresh_token",
            "refresh_token": saved["refresh_token"],
            "client_id": CLIENT_ID,
        },
    )
    token = load_json(data, "Token refresh")
    if status != 200 or not isinstance(token.get("access_token"), str):
        raise RuntimeError(f"Console token refresh failed with HTTP {status}; reconnect with device authorization")
    saved.update(
        access_token=token["access_token"],
        refresh_token=token["refresh_token"],
        expires_at=int(time.time()) + int(token["expires_in"]),
    )
    write_credentials(saved)
    return saved


def console_headers(saved: dict[str, Any]) -> dict[str, str]:
    headers = {"Authorization": "Bearer " + saved["access_token"], "Accept": "application/json"}
    if isinstance(saved.get("org_id"), str):
        headers["x-org-id"] = saved["org_id"]
    return headers


def command_console_config(args: argparse.Namespace) -> None:
    saved = credentials()
    status, _headers, data = request(saved["server"].rstrip("/") + "/api/v2/config", headers=console_headers(saved))
    if status == 401 and saved.get("refresh_token"):
        saved = refresh_credentials(saved)
        status, _headers, data = request(saved["server"].rstrip("/") + "/api/v2/config", headers=console_headers(saved))
    if status != 200:
        raise RuntimeError(f"Console config request failed with HTTP {status}: {response_text(data)[:2000]}")
    config = load_json(data, "Console config")
    safe_config = json.dumps(redact(config), ensure_ascii=False, indent=2).encode()
    save_artifact(args.save_dir, "console-config.json", safe_config)
    providers = config.get("providers", {}) if isinstance(config, dict) else {}
    for provider_id, provider in providers.items():
        print(f"provider={provider_id} package={provider.get('package', '[provider default]')}")
        raw_base_url = provider.get("settings", {}).get("baseURL", "[provider default]")
        base_url = safe_url(raw_base_url) if isinstance(raw_base_url, str) else "[provider default]"
        print(f"  baseURL={base_url}")
        for model_id, model in provider.get("models", {}).items():
            package = model.get("package", provider.get("package", '[provider default]'))
            protocol = "responses" if package == "@ai-sdk/openai" else "chat-completions" if package == "@ai-sdk/openai-compatible" else "unresolved"
            print(f"  model={model_id} modelID={model.get('modelID', model_id)} package={package} protocol={protocol}")
    if args.verbose:
        sanitized = redact(config)
        print(json.dumps(sanitized, indent=2, ensure_ascii=False))


def command_console_chat(args: argparse.Namespace) -> None:
    saved = credentials()
    if int(saved.get("expires_at", 0)) <= int(time.time()) and saved.get("refresh_token"):
        saved = refresh_credentials(saved)
    status, _headers, data = request(saved["server"].rstrip("/") + "/api/v2/config", headers=console_headers(saved))
    if status == 401 and saved.get("refresh_token"):
        saved = refresh_credentials(saved)
        status, _headers, data = request(saved["server"].rstrip("/") + "/api/v2/config", headers=console_headers(saved))
    if status != 200:
        raise RuntimeError(f"Console config request failed with HTTP {status}: {response_text(data)[:2000]}")
    config = load_json(data, "Console config")
    providers = config.get("providers", {})
    selection = None
    for provider_id, provider in providers.items():
        model = (provider.get("models") or {}).get(args.model)
        if model is not None:
            selection = provider_id, provider, model
            break
    if selection is None:
        raise RuntimeError(f"Console config does not define model {args.model}")
    provider_id, provider, model = selection
    if provider_id != "opencode":
        raise RuntimeError(
            f"Console model belongs to provider {provider_id}; this probe will not forward the Console access token to inference"
        )
    package = model.get("package", provider.get("package"))
    if isinstance(package, str) and package.startswith("aisdk:"):
        package = package.removeprefix("aisdk:")
    if package not in ("@ai-sdk/openai-compatible", "@ai-sdk/openai"):
        raise RuntimeError(f"Unsupported Console model package: {package}; no request was sent")
    base_url = provider.get("settings", {}).get("baseURL") or model.get("settings", {}).get("baseURL") or provider.get("api")
    if not isinstance(base_url, str):
        raise RuntimeError("Console config has no model base URL")
    model_id = model.get("modelID", args.model)
    url = openai_endpoint(base_url, {"npm": package})
    session = args.session_id or session_id()
    body = request_body(package, model_id, args.prompt, session)
    body = merge_overlay(body, provider.get("body", {}))
    body = merge_overlay(body, model.get("body", {}))
    headers = affinity_headers(session, args.project_id)
    headers["Authorization"] = "Bearer public"
    headers.update(provider.get("headers", {}))
    headers.update(model.get("headers", {}))
    if args.dry_run:
        redacted_request(url, headers, body)
        return
    code = stream_request(url, headers, body, args.raw, args.verbose, args.save_dir)
    if code == 401 and saved.get("refresh_token"):
        saved = refresh_credentials(saved)
        # The Console token authenticates config access, not inference. Refresh once and
        # recheck the server-provided provider config before retrying with the same request.
        retry_status, _headers, retry_data = request(
            saved["server"].rstrip("/") + "/api/v2/config", headers=console_headers(saved)
        )
        if retry_status != 200:
            raise RuntimeError(f"Console config retry failed with HTTP {retry_status}: {response_text(retry_data)[:2000]}")
        code = stream_request(url, headers, body, args.raw, args.verbose, args.save_dir)
    if not 200 <= code < 300:
        raise RuntimeError("Console inference returned a non-success status; no header variations were attempted")


def merge_overlay(base: dict[str, Any], overlay: Any) -> dict[str, Any]:
    if not isinstance(overlay, dict):
        return base
    result = dict(base)
    for key, value in overlay.items():
        if isinstance(result.get(key), dict) and isinstance(value, dict):
            result[key] = merge_overlay(result[key], value)
        else:
            result[key] = value
    return result


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    models = commands.add_parser("models", help="List free models in the public model catalog")
    models.add_argument("--save-dir")
    models.set_defaults(run=command_models)
    anon = commands.add_parser("anon-chat", help="Send a free-model request using the documented public credential")
    anon.add_argument("--model", required=True)
    anon.add_argument("--prompt", required=True)
    anon.add_argument("--session-id")
    anon.add_argument("--project-id")
    anon.add_argument("--dry-run", action="store_true")
    anon.add_argument("--raw", action="store_true")
    anon.add_argument("--verbose", action="store_true")
    anon.add_argument("--save-dir")
    anon.set_defaults(run=command_anon_chat)
    login = commands.add_parser("login", help="Connect this probe to the user's OpenCode Console account")
    login.set_defaults(run=command_login)
    config = commands.add_parser("console-config", help="Read provider configuration for the saved Console account")
    config.add_argument("--verbose", action="store_true")
    config.add_argument("--save-dir")
    config.set_defaults(run=command_console_config)
    chat = commands.add_parser("console-chat", help="Send a request using a Console-configured OpenAI protocol model")
    chat.add_argument("--model", required=True)
    chat.add_argument("--prompt", required=True)
    chat.add_argument("--session-id")
    chat.add_argument("--project-id")
    chat.add_argument("--dry-run", action="store_true")
    chat.add_argument("--raw", action="store_true")
    chat.add_argument("--verbose", action="store_true")
    chat.add_argument("--save-dir")
    chat.set_defaults(run=command_console_chat)
    return root


def main() -> int:
    args = parser().parse_args()
    try:
        args.run(args)
        return 0
    except (OSError, RuntimeError, KeyError, ValueError, urllib.error.URLError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
