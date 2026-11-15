from pathlib import Path
import sys

root = Path(sys.argv[1])
binary_directory = root / ".venv" / "bin"
files = [
    path
    for path in binary_directory.iterdir()
    if path.is_file() and not path.is_symlink()
]
staging_marker = b"/.vllm-staging-"
old_root = None

for path in files:
    data = path.read_bytes()
    first_line = data.splitlines()[0] if data else b""
    if first_line.startswith(b"#!") and staging_marker in first_line and b"/.venv/" in first_line:
        end = first_line.find(b"/.venv/", 2)
        old_root = first_line[2:end] if end > 2 else None
        break

if old_root is not None:
    new_root = str(root).encode()
    for path in files:
        data = path.read_bytes()
        rebased = data.replace(old_root, new_root)
        if rebased != data:
            path.write_bytes(rebased)
