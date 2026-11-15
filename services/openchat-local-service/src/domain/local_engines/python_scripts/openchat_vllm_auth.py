import hmac
import os

from starlette.responses import JSONResponse


class OpenChatAuthMiddleware:
    def __init__(self, app):
        self.app = app

    async def __call__(self, scope, receive, send):
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return

        token_path = os.environ.get("OPENCHAT_VLLM_AUTH_FILE")
        try:
            with open(token_path, "r", encoding="utf-8") as token_file:
                expected = token_file.read().strip()
        except (OSError, TypeError):
            expected = ""

        headers = dict(scope.get("headers", []))
        authorization = headers.get(b"authorization", b"").decode("latin-1")
        supplied = authorization.removeprefix("Bearer ")
        if not expected or not hmac.compare_digest(supplied, expected):
            response = JSONResponse(
                {
                    "error": {
                        "message": "Unauthorized",
                        "type": "authentication_error",
                    }
                },
                status_code=401,
            )
            await response(scope, receive, send)
            return

        await self.app(scope, receive, send)
