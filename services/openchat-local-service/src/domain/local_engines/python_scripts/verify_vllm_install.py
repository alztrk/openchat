from importlib.metadata import version
import sys

if version("vllm") != sys.argv[1]:
    raise SystemExit("The installed vLLM version does not match the pinned release.")
