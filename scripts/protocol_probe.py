#!/usr/bin/env python3
"""Verify that the image encoders emit the expected terminal protocols.

This runs ReddTTY in a raw PTY with a forced protocol and scans the byte
stream. It proves the encoder/render path, not that a specific GUI terminal
painted the image; GUI screenshot permission is unavailable in this environment.
"""

from __future__ import annotations

import fcntl
import json
import os
import pty
import select
import struct
import subprocess
import termios
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / "reddtty"
OUT = ROOT / "docs" / "verification"


def run(protocol: str) -> dict:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 140, 0, 0))
    env = dict(os.environ)
    env.update(
        {
            "TERM": "xterm-256color",
            "COLORTERM": "truecolor",
            "TERM_PROGRAM": "protocol-probe",
            "RUST_LOG": "reddtty=debug",
        }
    )
    env.pop("TMUX", None)
    env.pop("STY", None)
    config = f"/tmp/reddtty-probe-{protocol}.toml"
    try:
        proc = subprocess.Popen(
            [str(BIN), "--demo", "--images-protocol", protocol, "--config", config],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            cwd=str(ROOT),
            env=env,
            close_fds=True,
            start_new_session=True,
        )
    finally:
        os.close(slave)

    data = bytearray()
    deadline = time.time() + 5.0
    sent_view = False
    while time.time() < deadline:
        r, _, _ = select.select([master], [], [], 0.1)
        if master in r:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                break
            if not chunk:
                break
            data.extend(chunk)
            if len(data) > 8_000_000:
                del data[:-4_000_000]
        elapsed = 5.0 - (deadline - time.time())
        if not sent_view and elapsed > 1.2:
            # Move to the second post and open the full image viewer.
            os.write(master, b"j")
            time.sleep(0.1)
            os.write(master, b"v")
            sent_view = True
    try:
        proc.terminate()
        proc.wait(timeout=2)
    except Exception:
        proc.kill()
    os.close(master)

    raw = bytes(data)
    if protocol == "kitty":
        count = raw.count(b"\x1b_G")
        marker = count > 0
        detail = f"kitty_sequences={count}"
    elif protocol == "iterm2":
        count = raw.count(b"\x1b]1337;File=")
        marker = count > 0
        detail = f"iterm2_sequences={count}"
    elif protocol == "sixel":
        # Sixel is a DCS: ESC P <params> q "...". Params vary by encoder.
        count = raw.count(b"\x1bP9;1;0q") + raw.count(b"\x1bPq") + raw.count(b"\x1bP0;")
        marker = count > 0
        detail = f"sixel_sequences={count}"
    else:
        upper = raw.count("▀".encode())
        lower = raw.count("▄".encode())
        marker = upper + lower > 0
        detail = f"halfblock_cells={upper + lower}"
    return {
        "protocol": protocol,
        "ok": bool(marker),
        "detail": detail,
        "bytes": len(raw),
    }


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    results = [run(p) for p in ["kitty", "iterm2", "sixel", "halfblocks"]]
    (OUT / "protocol-probe.json").write_text(json.dumps(results, indent=2) + "\n")
    for result in results:
        print(result)
    return 0 if all(r["ok"] for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
