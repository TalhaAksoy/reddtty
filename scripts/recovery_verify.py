#!/usr/bin/env python3
"""Executable-level recovery and persistence-failure verification."""

from __future__ import annotations

import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / "reddtty"
SOCKET = "reddtty-recovery"
OUT = ROOT / "docs" / "verification"
OUT.mkdir(parents=True, exist_ok=True)
SUMMARY = {"assertions": [], "captures": []}


def tmux(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["tmux", "-L", SOCKET, *args], capture_output=True, text=True)


def capture(session: str) -> str:
    return tmux("capture-pane", "-p", "-t", session).stdout


def wait_for(session: str, needle: str, timeout: float = 12.0) -> str:
    deadline = time.time() + timeout
    text = ""
    while time.time() < deadline:
        text = capture(session)
        if needle in text:
            return text
        time.sleep(0.1)
    raise RuntimeError(f"timed out waiting for {needle!r}:\n{text}")


def assert_true(name: str, condition: bool, detail: str = "") -> None:
    SUMMARY["assertions"].append({"name": name, "ok": bool(condition), "detail": detail})
    if not condition:
        raise AssertionError(f"{name}: {detail}")


def send(session: str, key: str, literal: bool = False) -> None:
    args = ["send-keys", "-t", session]
    if literal:
        args.append("-l")
    args.append(key)
    tmux(*args)
    time.sleep(0.22)


def start(session: str, directory: Path, width: int = 100, height: int = 30) -> None:
    tmux("kill-session", "-t", session)
    cmd = (
        f"cd {ROOT} && {BIN} --demo --no-images --config {directory / 'config.toml'}; "
        "echo EXIT:$?; sleep 30"
    )
    result = tmux(
        "new-session", "-d", "-s", session, "-x", str(width), "-y", str(height), cmd
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr)
    wait_for(session, "Home")


def stop(session: str) -> None:
    send(session, "q")
    time.sleep(0.8)
    tmux("kill-session", "-t", session)


def save_capture(name: str, session: str) -> None:
    text = capture(session)
    path = OUT / f"recovery-{name}.txt"
    path.write_text(text)
    SUMMARY["captures"].append(str(path.relative_to(ROOT)))


def fresh_dir(tag: str) -> Path:
    return Path(tempfile.mkdtemp(prefix=f"reddtty-recovery-{tag}-"))


def backups(directory: Path, needle: str) -> list[Path]:
    return [
        path
        for path in directory.iterdir()
        if needle in path.name and path.is_file()
    ]


def corrupt_config(tag: str, bytes_: bytes) -> None:
    directory = fresh_dir(tag)
    path = directory / "config.toml"
    path.write_bytes(bytes_)
    start("cfg", directory)
    screen = capture("cfg")
    save_capture(f"{tag}-start", "cfg")
    lower = screen.lower()
    assert_true(
        f"{tag}: warning visible",
        "config" in lower or "could not read" in lower,
        screen[-500:],
    )
    send("cfg", "T")
    time.sleep(0.5)
    stop("cfg")
    assert_true(f"{tag}: original bytes preserved", path.read_bytes() == bytes_)
    found = backups(directory, ".corrupt-")
    assert_true(f"{tag}: corrupt backup exists", len(found) >= 1, str(found))
    assert_true(
        f"{tag}: backup has original bytes", found and found[0].read_bytes() == bytes_
    )
    shutil.rmtree(directory, ignore_errors=True)


def corrupt_library(tag: str, bytes_: bytes) -> None:
    directory = fresh_dir(tag)
    path = directory / "library.toml"
    path.write_bytes(bytes_)
    start("lib", directory)
    screen = capture("lib")
    save_capture(f"{tag}-start", "lib")
    lower = screen.lower()
    assert_true(
        f"{tag}: warning visible",
        "library" in lower or "could not read" in lower,
        screen[-500:],
    )
    send("lib", "b")
    time.sleep(0.5)
    stop("lib")
    assert_true(f"{tag}: original library bytes preserved", path.read_bytes() == bytes_)
    found = backups(directory, ".corrupt-")
    assert_true(f"{tag}: library backup exists", len(found) >= 1, str(found))
    shutil.rmtree(directory, ignore_errors=True)


def explicit_create_new() -> None:
    directory = fresh_dir("create-new")
    path = directory / "config.toml"
    original = b"not [ valid toml"
    path.write_bytes(original)
    start("new", directory)
    send("new", "S")
    wait_for("new", "Settings")
    for _ in range(7):
        send("new", "j")
    send("new", "Enter")
    wait_for("new", "Configuration recovery")
    save_capture("create-new-modal", "new")
    send("new", "j")
    send("new", "j")
    send("new", "Enter")
    time.sleep(1.0)
    tmux("kill-session", "-t", "new")
    assert_true("explicit create-new produced valid TOML", path.read_bytes() != original)
    with path.open("rb") as handle:
        data = tomllib.load(handle)
    assert_true("created config parses", "general" in data, str(data)[:200])
    assert_true(
        "original corrupt bytes still backed up",
        len(backups(directory, ".corrupt-")) >= 1,
    )
    shutil.rmtree(directory, ignore_errors=True)


def explicit_restore() -> None:
    directory = fresh_dir("restore")
    path = directory / "config.toml"
    backup = directory / "config.toml.bckp_1"
    restored = b'[general]\ntheme = "light"\n'
    path.write_bytes(b"not [ valid toml")
    backup.write_bytes(restored)
    start("restore", directory)
    send("restore", "S")
    wait_for("restore", "Settings")
    for _ in range(7):
        send("restore", "j")
    send("restore", "Enter")
    wait_for("restore", "Configuration recovery")
    send("restore", "j")
    send("restore", "Enter")
    time.sleep(1.0)
    tmux("kill-session", "-t", "restore")
    assert_true("explicit restore copied backup bytes", path.read_bytes() == restored)
    shutil.rmtree(directory, ignore_errors=True)


def write_failure() -> None:
    directory = fresh_dir("write-failure")
    start("fail", directory)
    os.chmod(directory, 0o555)
    try:
        send("fail", "b")
        screen = wait_for("fail", "Could not save", timeout=8)
        save_capture("write-failure", "fail")
        assert_true("write failure is visible", "Could not save" in screen, screen[-500:])
        assert_true("no false Saved toast", "Saved with an offline snapshot" not in screen)
        stop("fail")
    finally:
        os.chmod(directory, 0o755)
    library = directory / "library.toml"
    assert_true("failed write did not create a bookmark file", not library.exists())
    shutil.rmtree(directory, ignore_errors=True)


def main() -> int:
    if not BIN.exists():
        print(f"missing release binary: {BIN}", file=sys.stderr)
        return 2
    tmux("kill-server")
    try:
        corrupt_config("invalid-config", b"not [ valid toml")
        corrupt_config("truncated-config", b"[general]\ntheme = ")
        corrupt_config("nonutf8-config", b"\xff\xfe\x00\x80")
        corrupt_library("invalid-library", b"bookmarks = [oops")
        corrupt_library("nonutf8-library", b"\xff\xfe\x00\x80")
        explicit_create_new()
        explicit_restore()
        write_failure()
    finally:
        tmux("kill-server")
    summary = {
        "ok": all(item["ok"] for item in SUMMARY["assertions"]),
        "assertions": SUMMARY["assertions"],
        "captures": SUMMARY["captures"],
    }
    (OUT / "recovery-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps({"ok": summary["ok"], "assertions": len(summary["assertions"])}, indent=2))
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
