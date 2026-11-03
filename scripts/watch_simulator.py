"""Rebuild the simulator when a local build input changes."""

import json
import os
from pathlib import Path
import signal
import subprocess
import uuid


def subscribe():
    root = str(Path.cwd())
    subprocess.run(["watchman", "watch", root], check=True)
    expression = ["anyof",
        ["name", ["Cargo.toml", "Cargo.lock", "docs/ui.md", "docs/application-ui.md",
                  "docs/platform/toolkit.md"], "wholename"],
        ["allof", ["anyof", ["dirname", "crates"], ["dirname", "simulator"]],
         ["anyof", ["name", "Cargo.toml"], ["suffix", "rs"], ["suffix", "html"]]],
    ]
    client = subprocess.Popen(
        ["watchman", "-j", "-p", "--no-pretty"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
    )
    json.dump(["subscribe", root, "simulator", {"expression": expression, "fields": ["name"]}],
              client.stdin)
    client.stdin.close()
    return client


def stop(server):
    if server is not None and server.poll() is None:
        server.terminate()
        server.wait()


def main():
    server = None
    client = subscribe()
    try:
        for line in client.stdout:
            notice = json.loads(line)
            if "error" in notice or notice.get("canceled"):
                raise SystemExit(f"Watchman subscription failed: {notice}")
            if notice.get("files") or notice.get("is_fresh_instance"):
                result = subprocess.run([
                    "cargo", "build", "--locked", "--target-dir", "target",
                    "-p", "paper-mesh-simulator", "--bin", "paper-mesh-simulator",
                ], check=False)
                if result.returncode == 0:
                    stop(server)
                    server = subprocess.Popen(
                        ["target/debug/paper-mesh-simulator"],
                        env={**os.environ, "PAPER_MESH_GUIDE_BUILD": str(uuid.uuid4().int)},
                    )
                else:
                    print("Build failed. Waiting for a source change.", flush=True)
        raise SystemExit("Watchman disconnected.")
    finally:
        stop(client)
        stop(server)


def raise_exit(*_):
    raise SystemExit(0)


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, raise_exit)
    try:
        main()
    except KeyboardInterrupt:
        pass
