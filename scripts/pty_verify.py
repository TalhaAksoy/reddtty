#!/usr/bin/env python3
"""Drive ReddTTY through a real PTY (tmux) and capture what users see.

This is a verification harness, not a test that changes application state. It
writes plain terminal captures to docs/verification/ and a JSON summary.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / "reddtty"
OUT = ROOT / "docs" / "verification"
OUT.mkdir(parents=True, exist_ok=True)
CONFIG_DIR = Path(tempfile.mkdtemp(prefix="reddtty-pty-config-"))
CONFIG = CONFIG_DIR / "config.toml"
FEATURES_DIR = CONFIG_DIR / "features"
FEATURES_DIR.mkdir()
FEATURES_CONFIG = FEATURES_DIR / "config.toml"

SUMMARY: dict[str, object] = {"captures": [], "assertions": []}


def tmux(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["tmux", "-L", "reddtty-verify", *args], capture_output=True, text=True)


def capture(session: str) -> str:
    result = tmux("capture-pane", "-p", "-t", session)
    return result.stdout


def wait_for(session: str, needle: str, timeout: float = 12.0) -> str:
    deadline = time.time() + timeout
    text = ""
    while time.time() < deadline:
        text = capture(session)
        if needle in text:
            return text
        time.sleep(0.1)
    raise RuntimeError(f"timed out waiting for {needle!r}; last screen:\n{text}")


def assert_seen(name: str, text: str, needle: str) -> None:
    ok = needle in text
    SUMMARY["assertions"].append({"name": name, "needle": needle, "ok": ok})
    if not ok:
        raise AssertionError(f"{name}: missing {needle!r}")


def assert_not_seen(name: str, text: str, needle: str) -> None:
    ok = needle not in text
    SUMMARY["assertions"].append({"name": name, "needle": f"NOT {needle}", "ok": ok})
    if not ok:
        raise AssertionError(f"{name}: unexpectedly saw {needle!r}")


def save(name: str, text: str) -> None:
    path = OUT / f"{name}.txt"
    path.write_text(text)
    SUMMARY["captures"].append(str(path.relative_to(ROOT)))


def send(session: str, key: str, literal: bool = False) -> None:
    args = ["send-keys", "-t", session]
    if literal:
        args.append("-l")
    args.append(key)
    tmux(*args)
    time.sleep(0.18)


def start(width: int, height: int, name: str, config: Path = CONFIG) -> str:
    tmux("kill-session", "-t", name)
    cmd = (
        f"cd {ROOT} && {BIN} --demo --config {config}; "
        "echo EXIT:$?; sleep 20"
    )
    result = tmux("new-session", "-d", "-s", name, "-x", str(width), "-y", str(height), cmd)
    if result.returncode != 0:
        raise RuntimeError(result.stderr)
    wait_for(name, "REDDTTY")
    wait_for(name, "Best", timeout=12)
    return name


def main() -> int:
    if not BIN.exists():
        raise SystemExit(f"release binary missing: {BIN}")
    tmux("kill-server")
    tempdir = Path(tempfile.mkdtemp(prefix="reddtty-pty-"))
    try:
        # Main interaction flow at a typical large terminal.
        session = start(140, 38, "tl-flow")
        feed = capture(session)
        save("01-feed-140x38", feed)
        assert_seen("feed brand", feed, "REDDTTY")
        assert_seen("feed title", feed, "Home")
        assert_seen("demo label", feed, "DEMO")
        assert_seen("feed cards", feed, "▲")
        assert_seen("feed sort tabs", feed, "Controversial")
        assert_not_seen("feed not stuck loading", feed, "Loading posts…")

        # Left/Right move between the sidebar and the feed without Tab.
        send(session, "Left")
        sidebar_focus = capture(session)
        save("01b-sidebar-focus", sidebar_focus)
        assert_seen("left arrow focuses sidebar", sidebar_focus, "Favorite")
        send(session, "Right")
        feed_focus = capture(session)
        save("01c-feed-focus", feed_focus)
        assert_seen("right arrow returns to feed", feed_focus, "Search")

        # j then k verifies selection movement without changing the visible list.
        send(session, "j")
        send(session, "k")
        send(session, "Enter")
        detail = wait_for(session, "COMMENTS")
        save("02-post-detail", detail)
        assert_seen("post detail title", detail, "Why I love building CLI applications with Rust")
        assert_seen("post detail author", detail, "u/ferris_fan")
        assert_seen("post detail body", detail, "shipping")
        assert_seen("post detail comments", detail, "COMMENTS")
        assert_seen("post detail sort", detail, "Sort: Best")

        # Left/Right switch the post body and comments panes.
        send(session, "Left")
        body_focus = capture(session)
        save("03a-post-body-focus", body_focus)
        assert_seen("left arrow focuses post body", body_focus, "Reveal")
        send(session, "PageDown")
        body = capture(session)
        save("03-post-body-scrolled", body)
        assert_seen("body scroll indicator", body, "↕")

        # Back to comments, collapse a parent with replies.
        send(session, "Right")
        comments_focus = capture(session)
        save("03b-comments-focus", comments_focus)
        assert_seen("right arrow focuses comments", comments_focus, "Toggle")
        send(session, "j")
        send(session, "c")
        collapsed = capture(session)
        save("04-comments-collapsed", collapsed)
        assert_seen("comment collapse marker", collapsed, "▶")
        assert_seen("hidden reply count", collapsed, "replies hidden")
        send(session, "c")
        expanded = capture(session)
        save("05-comments-expanded", expanded)
        assert_not_seen("comment expansion removes hidden marker", expanded, "▶ u/")

        # Back to the feed, then open the selected image post in the gallery.
        send(session, "Escape")
        feed2 = wait_for(session, "posts")
        save("06-feed-return", feed2)
        assert_seen("returned feed", feed2, "Home")
        # Fifth Best post is the two-image black-hole gallery in demo data.
        for _ in range(5):
            send(session, "j")
        selected_gallery = capture(session)
        save("06b-gallery-post-selected", selected_gallery)
        assert_seen("gallery post selected", selected_gallery, "Visualising a black hole merger")
        send(session, "v")
        gallery = wait_for(session, "Image Viewer")
        save("07-gallery", gallery)
        assert_seen("gallery position", gallery, "1 / 2")
        assert_seen("gallery controls", gallery, "Esc close")
        send(session, "Right")
        gallery2 = capture(session)
        save("08-gallery-next", gallery2)
        assert_seen("gallery next position", gallery2, "2 / 2")
        send(session, "i")
        image_copy = wait_for(session, "local fixtures")
        save("08b-image-url-demo", image_copy)
        assert_seen("gallery image URL guard", image_copy, "local fixtures")
        send(session, "Escape")
        wait_for(session, "Home")

        # Sort menu, modal rendering and dismissal.
        send(session, "s")
        sort_menu = wait_for(session, "Top time range")
        save("09-sort-menu", sort_menu)
        assert_seen("sort menu range row", sort_menu, "Top time range")
        send(session, "Escape")

        # Go-to-subreddit typed input.
        send(session, "g")
        goto = wait_for(session, "Go to subreddit")
        send(session, "rust", literal=True)
        send(session, "Enter")
        rust = wait_for(session, "The Rust Programming Language")
        save("10-subreddit-rust", rust)
        assert_seen("subreddit title", rust, "r/rust")
        assert_seen("subreddit metadata", rust, "Rust")

        # Search typed input.
        send(session, "/")
        wait_for(session, "Search")
        send(session, "terminal", literal=True)
        send(session, "Tab")
        send(session, "Enter")
        search = wait_for(session, "⌕ terminal")
        save("11-search-results", search)
        assert_seen("search feed", search, "Search: terminal")

        # Quit and verify the alternate screen is left and shell output appears.
        send(session, "q")
        time.sleep(0.8)
        alt = tmux("display-message", "-p", "-t", session, "#{alternate_on}").stdout.strip()
        after_quit = capture(session)
        save("12-after-quit", after_quit)
        SUMMARY["assertions"].append({"name": "alternate screen left", "needle": alt, "ok": alt == "0"})
        assert_seen("shell resumed after quit", after_quit, "EXIT:0")
        tmux("kill-session", "-t", session)

        # Local-first features: bookmarks, read state, comment navigation,
        # source health, filters, command palette and restart-free config swap.
        session = start(140, 38, "tl-features", FEATURES_CONFIG)
        send(session, "b")
        bookmarked = wait_for(session, "★ saved")
        save("13-bookmark-marker", bookmarked)
        assert_seen("bookmark marker", bookmarked, "★ saved")
        send(session, "m")
        marked = wait_for(session, "Marked as read.")
        save("14-read-state", marked)
        assert_seen("read notification", marked, "Marked as read.")

        send(session, "B")
        bookmarks = wait_for(session, "Bookmarks")
        save("15-bookmarks-list", bookmarks)
        assert_seen("bookmarks modal", bookmarks, "Enter open")
        send(session, "Enter")
        wait_for(session, "COMMENTS")
        send(session, "Escape")
        wait_for(session, "Home")

        send(session, "Enter")
        wait_for(session, "COMMENTS")
        send(session, "z")
        collapsed = wait_for(session, "Collapsed all comment threads.")
        save("16-collapse-all", collapsed)
        assert_seen("collapse all", collapsed, "Collapsed all comment threads.")
        send(session, "z")
        wait_for(session, "Expanded all comment threads.")
        send(session, "n")
        unread = wait_for(session, "next unread comment")
        save("17-next-unread-comment", unread)
        assert_seen("next unread", unread, "next unread comment")
        send(session, "O")
        op_only = wait_for(session, "Showing only comments by")
        save("18-op-only", op_only)
        assert_seen("op only", op_only, "Showing only comments by")
        send(session, "O")
        wait_for(session, "Showing all comments.")
        send(session, "y")
        copied = wait_for(session, "copied to clipboard")
        save("19-copy-permalink", copied)
        assert_seen("copy permalink", copied, "copied to clipboard")
        send(session, "Escape")
        wait_for(session, "Home")

        send(session, "S")
        wait_for(session, "Settings")
        send(session, "j")
        send(session, "j")
        send(session, "Enter")
        health = wait_for(session, "Source health")
        save("20-source-health", health)
        assert_seen("source health title", health, "Source health")
        assert_seen("source health provider", health, "DEMO")
        assert_seen("source health retry", health, "Test now")
        assert_seen("source health change", health, "Change source")
        send(session, "r")
        time.sleep(0.8)
        testing = capture(session)
        save("21-source-retry", testing)
        assert_seen("source retry stays on screen", testing, "Source health")
        assert_seen("source retry action", testing, "Test now")

        send(session, "Escape")
        wait_for(session, "Settings")
        send(session, "j")
        send(session, "j")
        send(session, "j")
        send(session, "Enter")
        filters = wait_for(session, "Local filters")
        save("22-local-filters", filters)
        assert_seen("filters title", filters, "Local filters")
        assert_seen("filters hide read", filters, "Hide read posts")
        send(session, "a")
        wait_for(session, "Add local filter")
        send(session, "keyword:black hole", literal=True)
        send(session, "Enter")
        filtered = wait_for(session, "keyword  black hole")
        save("23-filter-added", filtered)
        assert_seen("filter rule", filtered, "keyword  black hole")
        send(session, "Escape")
        wait_for(session, "Settings")
        send(session, "Escape")
        hidden_feed = wait_for(session, "hidden by local filters")
        save("24-filtered-feed", hidden_feed)
        assert_not_seen("filter hides matching post", hidden_feed, "black hole merger")

        send(session, ":", literal=True)
        palette = wait_for(session, "Commands")
        save("25-command-palette", palette)
        assert_seen("command palette", palette, "Search posts")
        assert_seen("command palette themes", palette, "Theme: Catppuccin Mocha")
        send(session, "mocha", literal=True)
        send(session, "Enter")
        themed = wait_for(session, "Theme: Catppuccin Mocha")
        save("26-command-theme", themed)
        assert_seen("command theme applied", themed, "Theme: Catppuccin Mocha")

        send(session, "S")
        wait_for(session, "Settings")
        send(session, "Enter")
        picker = wait_for(session, "Choose config")
        save("27-config-picker", picker)
        assert_seen("config picker", picker, "Demo (offline)")
        send(session, "Enter")
        switched = wait_for(session, "Provider applied immediately")
        save("28-provider-switch", switched)
        assert_seen("provider switch", switched, "Provider applied immediately")
        assert_seen("provider still demo", switched, "DEMO")

        send(session, "q")
        time.sleep(0.8)
        after_features = capture(session)
        save("29-features-after-quit", after_features)
        assert_seen("features shell resumed", after_features, "EXIT:0")
        tmux("kill-session", "-t", session)

        # Responsive snapshots.
        for width, height in [(60, 20), (80, 24), (100, 30), (130, 36), (160, 45)]:
            name = f"tl-{width}x{height}"
            session = start(width, height, name)
            text = capture(session)
            save(f"size-{width}x{height}", text)
            assert_seen(f"{width}x{height} brand", text, "REDDTTY")
            assert_seen(f"{width}x{height} feed", text, "Home")
            assert_not_seen(f"{width}x{height} no loading", text, "Loading posts…")
            # The card content must never be drawn on top of the status bar.
            lines = text.rstrip("\n").split("\n")
            if lines:
                assert_seen(
                    f"{width}x{height} status bar",
                    lines[-1],
                    "Quit",
                )
            if width >= 60:
                send(session, "Enter")
                detail = wait_for(session, "COMMENTS")
                save(f"detail-{width}x{height}", detail)
                assert_seen(f"{width}x{height} detail title", detail, "Why I love")
                send(session, "Escape")
                wait_for(session, "Best")
                send(session, "?")
                help_screen = wait_for(session, "Keyboard Shortcuts")
                save(f"help-{width}x{height}", help_screen)
                assert_seen(f"{width}x{height} help close", help_screen, "Esc")
                send(session, "Escape")
                wait_for(session, "Best")
            tmux("send-keys", "-t", session, "q")
            time.sleep(0.6)
            after = capture(session)
            save(f"size-{width}x{height}-after-quit", after)
            assert_seen(f"{width}x{height} shell resumed", after, "EXIT:0")
            tmux("kill-session", "-t", session)

        # Tiny terminal: app must render and quit without a panic.
        session = start(30, 10, "tl-tiny")
        tiny = capture(session)
        save("size-30x10", tiny)
        assert_seen("tiny brand", tiny, "REDDTTY")
        assert_seen("tiny demo label", tiny, "DEMO")
        tmux("send-keys", "-t", session, "?")
        tiny_help = wait_for(session, "Keyboard Shortcuts")
        save("help-30x10", tiny_help)
        assert_seen("tiny help hidden-shortcut notice", tiny_help, "more")
        assert_seen("tiny help close hint", tiny_help, "Esc")
        tmux("send-keys", "-t", session, "Escape")
        wait_for(session, "Home")
        tmux("send-keys", "-t", session, "q")
        time.sleep(0.7)
        after_tiny = capture(session)
        save("size-30x10-after-quit", after_tiny)
        assert_seen("tiny shell resumed", after_tiny, "EXIT:0")
        tmux("kill-session", "-t", session)

        SUMMARY["ok"] = all(item["ok"] for item in SUMMARY["assertions"])  # type: ignore[index]
        (OUT / "pty-summary.json").write_text(json.dumps(SUMMARY, indent=2) + "\n")
        print(json.dumps({"ok": SUMMARY["ok"], "captures": len(SUMMARY["captures"]), "assertions": len(SUMMARY["assertions"])}, indent=2))
        return 0 if SUMMARY["ok"] else 1
    finally:
        for session in (
            "tl-flow",
            "tl-features",
            "tl-60x20",
            "tl-80x24",
            "tl-100x30",
            "tl-130x36",
            "tl-160x45",
            "tl-tiny",
        ):
            tmux("kill-session", "-t", session)
        shutil.rmtree(tempdir, ignore_errors=True)
        shutil.rmtree(CONFIG_DIR, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
