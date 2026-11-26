#!/usr/bin/env python3
"""Executable-level multi-process persistence verification.

Uses two real ReddTTY release processes sharing one isolated config directory.
Never touches the user's real config or library.
"""

from __future__ import annotations

import fcntl
import json
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / "reddtty"
SOCKET = "reddtty-process"
OUT = ROOT / "docs" / "verification"
OUT.mkdir(parents=True, exist_ok=True)
SUMMARY = {"assertions": [], "captures": []}


def tmux(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["tmux", "-L", SOCKET, *args], capture_output=True, text=True
    )


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
    raise RuntimeError(f"timed out waiting for {needle!r} in {session}:\n{text}")


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


def start(session: str, config_dir: Path, width: int = 140, height: int = 38) -> None:
    tmux("kill-session", "-t", session)
    cmd = (
        f"cd {ROOT} && {BIN} --demo --no-images --config {config_dir / 'config.toml'}; "
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


def library(config_dir: Path) -> dict:
    path = config_dir / "library.toml"
    if not path.exists():
        return {}
    with path.open("rb") as handle:
        return tomllib.load(handle)


def bookmark_ids(config_dir: Path) -> set[str]:
    return {item["post"]["id"] for item in library(config_dir).get("bookmarks", [])}


def read_ids(config_dir: Path) -> set[str]:
    return set(library(config_dir).get("read_posts", []))


def fresh_dir(tag: str) -> Path:
    path = Path(tempfile.mkdtemp(prefix=f"reddtty-process-{tag}-"))
    return path


def save_capture(name: str, session: str) -> None:
    text = capture(session)
    path = OUT / f"process-{name}.txt"
    path.write_text(text)
    SUMMARY["captures"].append(str(path.relative_to(ROOT)))


def test_adds(tag: str) -> None:
    directory = fresh_dir(tag)
    start("p1", directory)
    start("p2", directory)
    send("p1", "b")
    send("p2", "j")
    send("p2", "b")
    stop("p1")
    stop("p2")
    ids = bookmark_ids(directory)
    assert_true("two processes add different bookmarks", len(ids) == 2, str(ids))
    # Restart and verify the merged file is still readable.
    start("restart", directory)
    save_capture("restart", "restart")
    stop("restart")
    assert_true("restart keeps both bookmarks", bookmark_ids(directory) == ids)
    shutil.rmtree(directory, ignore_errors=True)


def test_remove_and_add(tag: str) -> None:
    directory = fresh_dir(tag)
    start("seed", directory)
    send("seed", "b")
    send("seed", "j")
    send("seed", "b")
    stop("seed")
    seeded = bookmark_ids(directory)
    assert_true("seed produced two bookmarks", len(seeded) == 2, str(seeded))
    start("remover", directory)
    start("adder", directory)
    send("remover", "b")  # removes first
    send("adder", "j")
    send("adder", "j")
    send("adder", "b")  # adds third
    stop("remover")
    stop("adder")
    final = bookmark_ids(directory)
    assert_true("removal survives concurrent add", len(final) == 2, str(final))
    assert_true("deleted bookmark is not resurrected", len(final - seeded) == 1, str(final))
    shutil.rmtree(directory, ignore_errors=True)


def test_read_and_filters(tag: str) -> None:
    directory = fresh_dir(tag)
    start("r1", directory)
    start("r2", directory)
    send("r1", "m")
    send("r2", "j")
    send("r2", "m")
    send("r1", ":", literal=True)
    send("r1", "filter add keyword:alpha", literal=True)
    send("r1", "Enter")
    send("r2", ":", literal=True)
    send("r2", "filter add keyword:beta", literal=True)
    send("r2", "Enter")
    stop("r1")
    stop("r2")
    reads = read_ids(directory)
    filters = library(directory).get("filters", {}).get("keywords", [])
    assert_true("concurrent read state merged", len(reads) == 2, str(reads))
    assert_true("concurrent filters merged", set(filters) == {"alpha", "beta"}, str(filters))
    shutil.rmtree(directory, ignore_errors=True)


def test_config_conflict(tag: str) -> None:
    directory = fresh_dir(tag)
    start("c1", directory)
    start("c2", directory)
    send("c1", "T")
    send("c2", "T")
    time.sleep(0.6)
    save_capture("config-conflict-c1", "c1")
    save_capture("config-conflict-c2", "c2")
    stop("c1")
    stop("c2")
    path = directory / "config.toml"
    assert_true("config exists after concurrent writes", path.exists())
    if path.exists():
        with path.open("rb") as handle:
            data = tomllib.load(handle)
        assert_true(
            "config remains valid TOML",
            data.get("general", {}).get("theme") in {"dark", "light"},
            str(data.get("general")),
        )
        assert_true("config revision advanced", int(data.get("revision", 0)) >= 1)
    shutil.rmtree(directory, ignore_errors=True)


def test_lock_timeout(tag: str) -> None:
    directory = fresh_dir(tag)
    start("seed", directory)
    send("seed", "b")
    stop("seed")
    before = bookmark_ids(directory)
    lock_path = directory / "library.lock"
    lock_file = lock_path.open("a+")
    fcntl.flock(lock_file, fcntl.LOCK_EX)
    try:
        start("blocked", directory)
        send("blocked", "j")
        send("blocked", "b")
        screen = wait_for("blocked", "Timed out", timeout=8)
        save_capture("lock-timeout", "blocked")
        assert_true("lock timeout is visible", "Timed out" in screen, screen[-400:])
        stop("blocked")
    finally:
        fcntl.flock(lock_file, fcntl.LOCK_UN)
        lock_file.close()
    assert_true("lock timeout leaves persisted data intact", bookmark_ids(directory) == before)
    # After releasing the lock a later process can save normally.
    start("after", directory)
    send("after", "j")
    send("after", "b")
    stop("after")
    assert_true("saving resumes after lock release", len(bookmark_ids(directory)) == 2)
    shutil.rmtree(directory, ignore_errors=True)


def test_crashed_lock_holder(tag: str) -> None:
    directory = fresh_dir(tag)
    start("seed", directory)
    send("seed", "b")
    stop("seed")
    lock_path = directory / "library.lock"
    holder = subprocess.Popen(
        [
            sys.executable,
            "-c",
            (
                "import fcntl,time,sys;"
                f"f=open({str(lock_path)!r},'a+');"
                "fcntl.flock(f,fcntl.LOCK_EX);"
                "print('locked',flush=True);time.sleep(60)"
            ),
        ],
        stdout=subprocess.PIPE,
        text=True,
    )
    assert holder.stdout is not None
    assert_true("lock holder started", holder.stdout.readline().strip() == "locked")
    holder.kill()
    holder.wait(timeout=5)
    start("after-crash", directory)
    send("after-crash", "j")
    send("after-crash", "b")
    stop("after-crash")
    assert_true(
        "OS releases a crashed process lock",
        len(bookmark_ids(directory)) == 2,
        str(bookmark_ids(directory)),
    )
    shutil.rmtree(directory, ignore_errors=True)


def main() -> int:
    if not BIN.exists():
        print(f"missing release binary: {BIN}", file=sys.stderr)
        return 2
    tmux("kill-server")
    try:
        test_adds("adds")
        test_remove_and_add("remove-add")
        test_read_and_filters("read-filter")
        test_config_conflict("config")
        test_lock_timeout("lock-timeout")
        test_crashed_lock_holder("crash-lock")
    finally:
        tmux("kill-server")
    summary = {
        "ok": all(item["ok"] for item in SUMMARY["assertions"]),
        "assertions": SUMMARY["assertions"],
        "captures": SUMMARY["captures"],
    }
    (OUT / "process-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps({"ok": summary["ok"], "assertions": len(summary["assertions"])}, indent=2))
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
