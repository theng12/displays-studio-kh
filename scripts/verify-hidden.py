#!/usr/bin/env python3
"""Run only under the machine-wide app-test lane; never drives macOS input."""
import json
import os
import selectors
import subprocess
import sys
import tempfile
import time
from pathlib import Path

binary = Path(sys.argv[1]).resolve(strict=True)
data = tempfile.mkdtemp(prefix="displays-studio-kh-")
print(f"Isolated test dir: {data}", flush=True)
with subprocess.Popen(
    [str(binary), "--webview-check", data],
    stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    start_new_session=True,
) as app:
    try:
        deadline = time.monotonic() + 35
        output = b""
        marker = None
        with selectors.DefaultSelector() as selector:
            selector.register(app.stdout, selectors.EVENT_READ)
            while time.monotonic() < deadline and selector.select(deadline - time.monotonic()):
                chunk = os.read(app.stdout.fileno(), 65536)
                if not chunk:
                    break
                output += chunk
                marker = next((line for line in output.split(b"\n")[:-1] if line.startswith(b"WEBVIEW_CHECK ")), None)
                if marker is not None:
                    break
            else:
                marker = None
        print(output.decode(errors="replace"), end="", flush=True)
        if not marker or not json.loads(marker.removeprefix(b"WEBVIEW_CHECK "))["ok"]:
            raise SystemExit("Hidden WKWebView did not pass its checks.")
        if app.poll() is not None:
            raise SystemExit(f"App exited instead of idling with its panel closed: {app.returncode}")
        print("Idle with panel closed, isolated mode (no visible tray); first CPU sample is since launch.", flush=True)
        subprocess.run(["/usr/bin/top", "-l", "4", "-s", "3", "-pid", str(app.pid), "-stats", "pid,cpu,mem,threads,time"], check=True, timeout=18)
        subprocess.run(["/bin/ps", "-p", str(app.pid), "-o", "pid=,rss=,comm="], check=True, timeout=5)
        if app.poll() is not None:
            raise SystemExit(f"App exited during idle sampling: {app.returncode}")
    finally:
        if app.poll() is None:
            app.terminate()
            app.wait(timeout=5)
