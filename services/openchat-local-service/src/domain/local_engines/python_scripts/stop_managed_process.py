import os
from pathlib import Path
import signal
import sys
import time

pid_path = Path(sys.argv[1])
marker = os.fsencode(sys.argv[2])
try:
    grace_period_seconds = float(sys.argv[3])
except ValueError as error:
    raise SystemExit("Invalid managed process shutdown timeout.") from error
if not 0 <= grace_period_seconds <= 30:
    raise SystemExit("Managed process shutdown timeout is out of range.")
raw_pid = pid_path.read_text().strip() if pid_path.exists() else ""
pid = int(raw_pid) if raw_pid.isdecimal() else 0
process_file = Path("/proc") / str(pid) / "cmdline"


def is_managed_process() -> bool:
    if pid <= 1:
        return False
    try:
        return marker in process_file.read_bytes().split(b"\0")
    except OSError:
        return False


if is_managed_process():
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    deadline = time.monotonic() + grace_period_seconds
    while time.monotonic() < deadline and is_managed_process():
        time.sleep(0.1)
    if is_managed_process():
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass

pid_path.unlink(missing_ok=True)
