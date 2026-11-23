#!/usr/bin/env python3
"""Generate ReddTTY demo fixtures.

The fixtures are deterministic so tests can rely on scores and ordering.
Images are plain RGB PNGs produced with only the Python standard library.
"""

from __future__ import annotations

import json
import math
import os
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIX = ROOT / "fixtures"
IMG = FIX / "images"
IMG.mkdir(parents=True, exist_ok=True)

BASE = 1_764_000_000  # arbitrary stable epoch; the demo provider shifts it at runtime
SUBS = ["rust", "linux", "programming", "technology", "opensource", "unixporn", "webdev", "machinelearning", "askscience", "gaming"]
NOW_AGE = {}

POSTS = [
    dict(id="demo001", sub="rust", author="ferris_fan", title="Why I love building CLI applications with Rust",
         age=180, score=3240, comments=248, kind="text", flair="Discussion", sticky=False,
         body="""I have been writing command-line tools for a decade, and Rust is the first language where **shipping** feels this good.

## What changed for me

* A single static binary is easy to hand to someone else
* `Result` makes failure paths visible instead of optional
* Cargo's defaults are boring in the best possible way
* The ecosystem has excellent crates for terminals, parsing and HTTP

> The compiler is not fighting you. It is reviewing your work before your users do.

Here is the smallest argument parser I actually enjoy using:

```rust
#[derive(clap::Parser)]
struct Args {
    /// Subreddit to open on launch
    #[arg(short, long)]
    subreddit: Option<String>,
}
```

The full article covers packaging, cross-compilation and a few mistakes I made with async cleanup. Ask me anything about the setup.
"""),
    dict(id="demo002", sub="rust", author="oxide_orchard", title="Rust 2026: my terminal setup after three years",
         age=30, score=1240, comments=86, kind="image", flair="Show & Tell", sticky=False,
         image="aurora.png", body="Three years of tweaking, and I finally stopped changing my prompt every week."),
    dict(id="demo003", sub="linux", author="quiet_daemon", title="I switched my entire workflow to Linux",
         age=540, score=2440, comments=187, kind="image", flair="Story", sticky=False,
         image="mountains.png", body="After using another OS for years, I moved my whole workflow over a long weekend. The surprises were smaller than I expected."),
    dict(id="demo004", sub="linux", author="shell_poet", title="What's the one Linux tool you can't live without?",
         age=120, score=980, comments=412, kind="text", flair="Question", sticky=False,
         body="For me it is `ripgrep`. It replaced three scripts and half of my muscle memory.\n\nWhat is yours, and why?"),
    dict(id="demo005", sub="programming", author="clean_code_ish", title="The hidden cost of premature abstraction",
         age=300, score=1860, comments=143, kind="link", flair="Article", sticky=False,
         url="https://example.com/premature-abstraction", thumb="terminal.png", domain="example.com",
         body="A good read on why the second use case is often too early to design for."),
    dict(id="demo006", sub="technology", author="packet_garden", title="A new open standard for portable terminal graphics",
         age=1440, score=1540, comments=97, kind="link", flair="News", sticky=False,
         url="https://example.com/terminal-graphics", thumb="city.png", domain="example.com"),
    dict(id="demo007", sub="opensource", author="tiny_commits", title="Open source UI kit, five themes, zero telemetry",
         age=420, score=876, comments=64, kind="gallery", flair="Release", sticky=False,
         images=["aurora.png", "waves.png", "city.png"], body="After a year of weekend work the component kit is finally public. Feedback very welcome."),
    dict(id="demo008", sub="unixporn", author="rice_enjoyer", title="My first tiling WM rice",
         age=40, score=632, comments=58, kind="image", flair="Screenshot", sticky=False,
         image="desktop.png"),
    dict(id="demo009", sub="unixporn", author="pixel_quiet", title="Minimal desktop, maximum focus",
         age=2880, score=1180, comments=74, kind="gallery", flair="Screenshot", sticky=False,
         images=["desktop.png", "terminal.png", "coffee.png"], body="Two monitors, one palette, no icons."),
    dict(id="demo010", sub="webdev", author="cascade_cartographer", title="CSS in 2026 is finally fun again",
         age=240, score=720, comments=61, kind="text", flair="Discussion", sticky=False,
         body="Container queries, native nesting and a sane color model changed how I write styles. What are you using in production?"),
    dict(id="demo011", sub="machinelearning", author="gradient_descent", title="A readable survey of small language models",
         age=780, score=910, comments=39, kind="link", flair="Paper", sticky=False,
         url="https://example.com/small-language-models", thumb="tall.png", domain="example.com"),
    dict(id="demo012", sub="machinelearning", author="laptop_trainer", title="I trained a tiny model on a laptop",
         age=4320, score=1350, comments=112, kind="text", flair="Project", sticky=False,
         body="""No cluster, no rented GPUs, just a very patient laptop and a small dataset.

- 14M parameters
- 9 hours of training
- one very warm lap

The point was not the benchmark. The point was understanding every part of the pipeline."""),
    dict(id="demo013", sub="rust", author="borrow_live", title="Live coding: writing a TUI in 30 minutes",
         age=360, score=560, comments=44, kind="video", flair="Video", sticky=False,
         url="https://example.com/watch/tui-live", thumb="terminal.png", duration=1840),
    dict(id="demo014", sub="programming", author="diff_whisperer", title="What makes a good code review?",
         age=1320, score=640, comments=88, kind="video", flair="Video", sticky=False,
         url="https://example.com/watch/code-review", thumb="coffee.png", duration=1220,
         body="A short talk I keep coming back to. The best reviews are about shared understanding, not style points."),
    dict(id="demo015", sub="linux", author="old_hardware", title="Linux on a 2009 laptop in 2026",
         age=7200, score=1890, comments=156, kind="video", flair="Video", sticky=False,
         url="https://example.com/watch/old-laptop", thumb="desktop.png", duration=930),
    dict(id="demo016", sub="opensource", author="maintainer_maria", title="We just hit 10k stars — lessons learned",
         age=480, score=1020, comments=93, kind="text", flair="Meta", sticky=False,
         body="""Stars are not users, but they are a useful excuse to write down what we learned.

1. Write the CONTRIBUTING file before you need it.
2. Say no to features that do not have a maintainer.
3. Automate releases early.
4. Thank people publicly.

The biggest surprise was how much documentation reduced support load."""),
    dict(id="demo017", sub="technology", author="local_first", title="Why local-first software is winning",
         age=1440, score=840, comments=71, kind="text", flair="Essay", sticky=False,
         body="Latency is a feature. Ownership is a feature. Being able to work on a train is a feature."),
    dict(id="demo018", sub="askscience", author="wave_function", title="How do noise-cancelling headphones actually work?",
         age=600, score=760, comments=53, kind="text", flair="Physics", sticky=False,
         body="I understand the idea of inverted waves, but how does that work for unpredictable noise like a conversation?"),
    dict(id="demo019", sub="askscience", author="orbit_nerd", title="A new image of a planet-forming disk",
         age=5760, score=2010, comments=102, kind="link", flair="Space", sticky=False,
         url="https://example.com/planet-disk", thumb="aurora.png", domain="example.com"),
    dict(id="demo020", sub="gaming", author="cozy_speedrun", title="My cozy indie game corner",
         age=25, score=420, comments=31, kind="image", flair="Setup", sticky=False,
         image="coffee.png", body="Rain outside, warm lamp, and a very small farming game."),
    dict(id="demo021", sub="gaming", author="pixel_picnic", title="Six screenshots from a game I've been making",
         age=8640, score=1580, comments=119, kind="gallery", flair="Indie Dev", sticky=False,
         images=["city.png", "waves.png", "aurora.png", "desktop.png", "mountains.png"],
         body="Everything is placeholders except the sky, which I am irrationally proud of."),
    dict(id="demo022", sub="rust", author="roadmap_watcher", title="Rust 2026 roadmap discussion",
         age=120, score=410, comments=77, kind="link", flair="Discussion", sticky=False,
         url="https://example.com/rust-roadmap-2026", thumb="terminal.png", domain="example.com"),
    dict(id="demo023", sub="rust", author="wasm_weekend", title="[Update] My crate now supports WASM",
         age=720, score=330, comments=28, kind="text", flair="Project Update", sticky=False,
         body="Thanks for all the feedback on the first release. The browser build now passes the same test suite, minus file-system features."),
    dict(id="demo024", sub="linux", author="compositor_curious", title="Wayland compositor comparison, 2026 edition",
         age=11520, score=1120, comments=134, kind="link", flair="Guide", sticky=False,
         url="https://example.com/wayland-2026", thumb="desktop.png", domain="example.com"),
    dict(id="demo025", sub="programming", author="module_boundary", title="Stop writing utils.rs",
         age=180, score=1520, comments=205, kind="text", flair="Opinion", sticky=False,
         body="""A file called `utils` is a symptom, not a solution. It means you have not decided what the code is *for*.

Try naming modules after the nouns and verbs in your domain instead. Your future self will thank you."""),
    dict(id="demo026", sub="webdev", author="gradient_goblin", title="A tiny CSS gradient generator",
         age=15, score=260, comments=18, kind="image", flair="Tool", sticky=False,
         image="waves.png", body="No accounts, no analytics, one HTML file."),
    dict(id="demo027", sub="technology", author="risky_click", title="The most cursed cable management you will see today",
         age=1080, score=190, comments=47, kind="image", flair="NSFW", sticky=False,
         image="terminal.png", body="Marked NSFW by the mods. Honestly, fair.", nsfw=True),
    dict(id="demo028", sub="programming", author="spoiler_alert", title="Spoiler: the bug was in the build script",
         age=1800, score=470, comments=36, kind="text", flair="Debugging", sticky=False,
         spoiler=True, body="I spent six hours reading the parser source. The actual problem was a stale generated file."),
    dict(id="demo029", sub="linux", author="gone_user", title="[Removed by moderators]",
         age=240, score=0, comments=0, kind="removed", flair=None, sticky=False, removed=True),
    dict(id="demo030", sub="opensource", author="history_buff", title="Contribution graph of a 20-year-old project",
         age=17280, score=990, comments=62, kind="image", flair="Data", sticky=False,
         image="city.png", body="Every square is someone's afternoon."),
    dict(id="demo031", sub="askscience", author="event_horizon", title="Visualising a black hole merger",
         age=4320, score=1770, comments=83, kind="gallery", flair="Astronomy", sticky=False,
         images=["tall.png", "aurora.png"], body="Four frames from a simulation, plus the article that explains the colours."),
    dict(id="demo032", sub="gaming", author="backlog_bard", title="A cozy building game finally left early access",
         age=12960, score=830, comments=57, kind="link", flair="Release", sticky=False,
         url="https://example.com/cozy-build", thumb="coffee.png", domain="example.com"),
    dict(id="demo033", sub="unixporn", author="portrait_pixels", title="Portrait monitor workflow",
         age=45, score=355, comments=22, kind="image", flair="Screenshot", sticky=False,
         image="tall.png"),
    dict(id="demo034", sub="rust", author="mod_ferris", title="Weekly help thread — ask anything",
         age=60, score=52, comments=301, kind="text", flair="Moderator", sticky=True,
         body="New to Rust? Stuck on a lifetime? Ask here. No question is too small."),
    dict(id="demo035", sub="programming", author="weekend_builder", title="What are you working on this weekend?",
         age=300, score=380, comments=164, kind="text", flair="Weekly", sticky=False,
         body="Share what you are building, learning or breaking this weekend."),
    dict(id="demo036", sub="technology", author="policy_watcher", title="EU interoperability rules explained without the jargon",
         age=28800, score=690, comments=88, kind="link", flair="Policy", sticky=False,
         url="https://example.com/eu-interop", thumb="city.png", domain="example.com"),
    dict(id="demo037", sub="linux", author="sunset_terminal", title="Sunset over my terminal",
         age=30, score=290, comments=14, kind="image", flair="Screenshot", sticky=False,
         image="mountains.png", body="The wallpaper changes with the time of day."),
    dict(id="demo038", sub="opensource", author="review_rotation", title="How we review 200 PRs a month without burning out",
         age=86400, score=1210, comments=76, kind="text", flair="Process", sticky=False,
         body="Three things helped: a review rota, a small checklist, and permission to say 'not this week'."),
    dict(id="demo039", sub="webdev", author="layout_lab", title="Five palettes from one design system",
         age=420, score=505, comments=41, kind="gallery", flair="Design", sticky=False,
         images=["waves.png", "city.png", "aurora.png", "mountains.png", "terminal.png"],
         body="Same components, five moods. Which one would you ship?"),
    dict(id="demo040", sub="rust", author="ferris_doodle", title="Ferris in a terminal",
         age=4320, score=640, comments=33, kind="image", flair="Art", sticky=False,
         image="coffee.png"),
]

SUBREDDITS = [
    dict(name="rust", title="The Rust Programming Language", subscribers=412_000, active=8_400,
         description="A place for all things Rust: news, questions, crates and the occasional Ferris drawing.",
         created_utc=BASE - 4_500 * 86_400),
    dict(name="linux", title="Linux, GNU and open source", subscribers=1_920_000, active=22_000,
         description="News, tips and stories from the Linux community. Beginners welcome.",
         created_utc=BASE - 6_900 * 86_400),
    dict(name="programming", title="Programming", subscribers=6_400_000, active=41_000,
         description="Computer programming: articles, discussion and the craft of building software.",
         created_utc=BASE - 7_100 * 86_400),
    dict(name="technology", title="Technology", subscribers=16_800_000, active=63_000,
         description="The latest technology news and analysis from around the world.",
         created_utc=BASE - 7_000 * 86_400),
    dict(name="opensource", title="Open Source", subscribers=248_000, active=3_200,
         description="Projects, maintainers and the people who keep open source running.",
         created_utc=BASE - 5_800 * 86_400),
    dict(name="unixporn", title="Unixporn", subscribers=690_000, active=4_100,
         description="Screenshots of beautiful, efficient and unusual desktop setups.",
         created_utc=BASE - 5_400 * 86_400),
    dict(name="webdev", title="Web Development", subscribers=2_300_000, active=18_000,
         description="A community for web developers of every level.",
         created_utc=BASE - 6_200 * 86_400),
    dict(name="machinelearning", title="Machine Learning", subscribers=3_100_000, active=12_000,
         description="Papers, projects and practical discussion about machine learning.",
         created_utc=BASE - 5_600 * 86_400),
    dict(name="askscience", title="Ask a Science Question", subscribers=26_000_000, active=14_000,
         description="Got a question about science? Ask experts and enthusiasts.",
         created_utc=BASE - 6_500 * 86_400),
    dict(name="gaming", title="Gaming", subscribers=42_000_000, active=52_000,
         description="A community for everything gaming, from AAA to tiny indie experiments.",
         created_utc=BASE - 6_800 * 86_400),
]

BODIES = [
    "This matches my experience. The hard part was not the syntax, it was unlearning old habits.",
    "I tried this last month and the difference was immediate. Thanks for writing it up.",
    "Counterpoint: the simple version is easier to debug at 2am. We should be careful about adding layers.",
    "Could you share the benchmark you used? I would like to reproduce it on a smaller machine.",
    "The part about documentation is underrated. Half of my bookmarks are just good READMEs.",
    "I have been using this approach in production for a year. One caveat: watch the memory usage.",
    "This is the clearest explanation I have read. Saving it for the next code review.",
    "I disagree with the conclusion, but the reasoning is solid. Upvoted for the discussion.",
    "Does this work with older terminals? I spend a lot of time on servers over SSH.",
    "Beautiful write-up. The diagrams alone are worth the read.",
    "I ran into the same bug. It turned out to be a stale cache, not the library.",
    "Small correction: the flag is documented in the second section, not the first.",
    "This is why I love small tools. One job, done well, composable with everything else.",
    "The examples are great, but I would add a warning about running this as root.",
    "Honestly the comments here are more useful than most blog posts. Thanks everyone.",
]
AUTHORS = ["byte_wanderer", "quiet_signal", "kernel_kate", "tab_width", "small_steps", "linus_lite",
           "async_alice", "no_semicolons", "coffee_compiler", "firefox_fox", "glyph_gardener", "null_pointer"]

def lcg(seed: int):
    state = seed & 0xFFFFFFFF
    def nxt() -> int:
        nonlocal state
        state = (1103515245 * state + 12345) & 0x7FFFFFFF
        return state
    return nxt

def comment_tree(post: dict, index: int):
    rnd = lcg(1000 + index * 7919)
    post_created = BASE - post["age"] * 60
    counter = [0]
    def uid():
        counter[0] += 1
        return f"{post['id']}_c{counter[0]}"

    def make(depth: int, max_depth: int):
        cid = uid()
        author = AUTHORS[rnd() % len(AUTHORS)]
        body = BODIES[rnd() % len(BODIES)]
        score = max(-12, int(1200 / (depth + 1) * (0.3 + (rnd() % 100) / 100)))
        created = min(BASE - 60, post_created + counter[0] * 180)
        node = {
            "id": cid,
            "parent_id": None,
            "author": author,
            "body": body,
            "score": score,
            "created_utc": created,
            "distinguished": "moderator" if rnd() % 17 == 0 else None,
            "deleted": False,
            "removed": False,
            "is_more": False,
            "more_count": 0,
            "more_children": [],
            "replies": [],
        }
        if depth > 0 and rnd() % 9 == 0:
            node["deleted"] = True
            node["author"] = "[deleted]"
            node["body"] = "[deleted]"
        if depth < max_depth and rnd() % 100 < 70:
            child_count = 1 + (rnd() % 2)
            for _ in range(child_count):
                child = make(depth + 1, max_depth)
                child["parent_id"] = cid
                node["replies"].append(child)
        return node

    roots = []
    root_count = 3 + (index % 4)
    if post["id"] == "demo001":
        root_count = 5
    for i in range(root_count):
        max_depth = 6 if post["id"] == "demo001" and i == 0 else (3 if i % 2 == 0 else 1)
        node = make(0, max_depth)
        roots.append(node)

    if post["id"] in ("demo001", "demo004", "demo016", "demo025"):
        more_count = 4 + (index % 3)
        children = [f"{post['id']}_more_{i}" for i in range(more_count)]
        stub = {
            "id": f"{post['id']}_more_stub",
            "parent_id": roots[-1]["id"],
            "author": "[more]",
            "body": "",
            "score": 0,
            "created_utc": post_created,
            "distinguished": None,
            "deleted": False,
            "removed": False,
            "is_more": True,
            "more_count": more_count,
            "more_children": children,
            "replies": [],
        }
        roots[-1]["replies"].append(stub)

    return roots

def write_png(path: Path, width: int, height: int, fn):
    raw = bytearray()
    for y in range(height):
        raw.append(0)
        for x in range(width):
            r, g, b = fn(x, y, width, height)
            raw.extend((max(0, min(255, int(r))), max(0, min(255, int(g))), max(0, min(255, int(b)))))
    def chunk(tag: bytes, data: bytes):
        c = struct.pack(">I", len(data)) + tag + data
        c += struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        return c
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    path.write_bytes(png)

def sky(x, y, w, h):
    t = y / max(1, h - 1)
    r = 18 + 100 * t
    g = 24 + 80 * t
    b = 70 + 120 * t
    return r, g, b

def aurora(x, y, w, h):
    t = y / max(1, h - 1)
    r, g, b = sky(x, y, w, h)
    band = math.sin(x / 53.0 + t * 4.0) * 0.5 + 0.5
    band2 = math.sin(x / 21.0 - t * 7.0) * 0.5 + 0.5
    glow = max(0.0, 1.0 - abs(t - 0.42 - band * 0.16) * 3.4)
    r += glow * (40 + 50 * band2)
    g += glow * (190 + 50 * band)
    b += glow * (110 + 60 * band2)
    stars = ((x * 92821 + y * 68917) % 9973) < 4 and t < 0.55
    if stars:
        r = g = b = 235
    if t > 0.78:
        mountain = 0.78 + 0.05 * math.sin(x / 40.0)
        if t > mountain:
            shade = 0.5 + 0.5 * math.sin(x / 11.0)
            r, g, b = 24 + shade * 12, 30 + shade * 16, 42 + shade * 20
    return r, g, b

def mountains(x, y, w, h):
    r, g, b = sky(x, y, w, h)
    sun_x, sun_y = w * 0.72, h * 0.30
    d = math.hypot(x - sun_x, y - sun_y)
    if d < 34:
        glow = max(0.0, 1 - d / 34)
        r += glow * 120
        g += glow * 80
        b += glow * 20
    for i, (base, color) in enumerate([(0.58, (52, 61, 84)), (0.68, (36, 44, 62)), (0.78, (22, 28, 40))]):
        ridge = base + 0.07 * math.sin(x / (34 + i * 11) + i * 2.0) + 0.03 * math.sin(x / 9.0)
        if y / h > ridge:
            r, g, b = color
    return r, g, b

def terminal(x, y, w, h):
    r, g, b = 14, 17, 20
    if 20 <= x < w - 20 and 24 <= y < h - 24:
        r, g, b = 22, 26, 31
    if 20 <= x < w - 20 and 24 <= y < 48:
        r, g, b = 32, 37, 44
    for i, c in enumerate([(240, 95, 80), (245, 190, 80), (110, 200, 120)]):
        if 32 + i * 20 < x < 40 + i * 20 and 32 < y < 40:
            r, g, b = c
    line = (y - 60) // 26
    if 42 <= x < w - 42 and 64 <= y < h - 40 and 0 <= line < 12 and (line * 37 + 11) % 5 != 0:
        length = 60 + ((line * 131 + x * 3) % 340)
        if x < 42 + length:
            if (line + x // 7) % 11 == 0:
                r, g, b = 245, 160, 90
            elif (line * 3 + x // 9) % 7 == 0:
                r, g, b = 100, 200, 150
            else:
                r, g, b = 150, 160, 175
    return r, g, b

def city(x, y, w, h):
    t = y / max(1, h - 1)
    r = 10 + 40 * t
    g = 16 + 30 * t
    b = 40 + 80 * t
    if t > 0.55:
        r, g, b = 12, 14, 22
    for bx in range(0, 9):
        left = int(bx * w / 9)
        width = int(w / 9) + 2
        height = int(h * (0.18 + ((bx * 37) % 45) / 100))
        top = h - height
        if y > top and left <= x < left + width:
            r, g, b = 20 + bx * 3, 24 + bx * 2, 38 + bx * 4
            if (x % 12 < 5) and (y % 16 < 7) and (x + y + bx) % 7 != 0:
                r, g, b = 245, 190, 95
    return r, g, b

def waves(x, y, w, h):
    t = y / max(1, h - 1)
    r = 12 + 10 * math.sin(x / 60)
    g = 40 + 70 * t
    b = 90 + 100 * t
    for i in range(7):
        wy = h * (0.18 + i * 0.12) + math.sin(x / (45 + i * 7) + i) * (10 + i * 2)
        if abs(y - wy) < 4:
            r += 60
            g += 50
            b += 30
    return r, g, b

def desktop(x, y, w, h):
    r, g, b = 20, 22, 28
    if y < 22:
        r, g, b = 30, 33, 40
    if y > h - 46:
        r, g, b = 26, 28, 35
    if 18 <= x < w // 2 and 38 <= y < h // 2:
        r, g, b = 30, 36, 46
        if 26 <= x < w // 2 - 8 and 46 <= y < h // 2 - 14 and (x * y) % 97 < 20:
            r, g, b = 90, 160, 200
    if w // 2 + 12 <= x < w - 18 and 38 <= y < h // 2:
        r, g, b = 32, 38, 48
    for i in range(5):
        cx = w // 2 - 90 + i * 42
        if cx <= x < cx + 28 and h - 32 <= y < h - 6:
            r, g, b = 50 + i * 20, 70 + i * 10, 100
    return r, g, b

def coffee(x, y, w, h):
    t = y / max(1, h - 1)
    r = 70 + 100 * t
    g = 38 + 60 * t
    b = 28 + 35 * t
    cx, cy = w * 0.5, h * 0.62
    d = math.hypot((x - cx) / 1.0, (y - cy) * 1.1)
    if d < 105:
        r, g, b = 235, 225, 205
    if d < 84:
        r, g, b = 90, 52, 34
        if d < 68:
            r, g, b = 62, 36, 24
    for i in range(3):
        sx = cx - 40 + i * 40 + math.sin(y / 16 + i) * 8
        if abs(x - sx) < 5 and cy - 190 < y < cy - 110:
            r, g, b = 240, 235, 225
    return r, g, b

def tall(x, y, w, h):
    t = y / max(1, h - 1)
    r = 30 + 160 * t
    g = 60 + 100 * (1 - t)
    b = 90 + 120 * math.sin(t * math.pi)
    stripe = (x // 8) % 2
    if stripe:
        r *= 0.85
        g *= 0.9
        b *= 0.95
    if abs(x - w / 2) < 3:
        r, g, b = 240, 240, 230
    return r, g, b

IMAGES = {
    "aurora.png": (640, 400, aurora),
    "mountains.png": (640, 400, mountains),
    "terminal.png": (640, 400, terminal),
    "city.png": (640, 400, city),
    "waves.png": (640, 400, waves),
    "desktop.png": (640, 400, desktop),
    "coffee.png": (640, 400, coffee),
    "tall.png": (420, 630, tall),
}

def main():
    posts = []
    comments = {}
    for index, p in enumerate(POSTS):
        kind = p["kind"]
        content = None
        if kind == "text":
            content = {"kind": "text"}
        elif kind == "image":
            content = {"kind": "image", "url": f"demo://img/{p['image']}"}
        elif kind == "gallery":
            content = {"kind": "gallery", "items": [{"url": f"demo://img/{n}", "width": None, "height": None} for n in p["images"]]}
        elif kind == "link":
            content = {"kind": "link", "url": p["url"], "thumbnail": f"demo://img/{p['thumb']}" if p.get("thumb") else None}
        elif kind == "video":
            content = {"kind": "video", "url": p["url"], "thumbnail": f"demo://img/{p['thumb']}" if p.get("thumb") else None, "duration": p.get("duration")}
        elif kind == "removed":
            content = {"kind": "removed"}
        else:
            raise ValueError(kind)

        post = {
            "id": p["id"],
            "subreddit": p["sub"],
            "author": p["author"],
            "title": p["title"],
            "body": p.get("body", ""),
            "created_utc": BASE - p["age"] * 60,
            "score": p["score"],
            "num_comments": p["comments"] if kind != "removed" else 0,
            "permalink": f"/r/{p['sub']}/comments/{p['id']}/{p['title'].lower().replace(' ', '_')[:50]}/",
            "content": content,
            "flair": p.get("flair"),
            "nsfw": bool(p.get("nsfw", False)),
            "spoiler": bool(p.get("spoiler", False)),
            "stickied": bool(p.get("sticky", False)),
            "locked": False,
            "removed": bool(p.get("removed", False)),
            "is_self": kind in ("text", "removed"),
            "domain": p.get("domain"),
            "thumbnail": None,
        }
        posts.append(post)
        comments[p["id"]] = comment_tree(p, index)

    (FIX / "posts.json").write_text(json.dumps(posts, indent=2, ensure_ascii=False) + "\n")
    (FIX / "comments.json").write_text(json.dumps(comments, indent=2, ensure_ascii=False) + "\n")
    (FIX / "subreddits.json").write_text(json.dumps(SUBREDDITS, indent=2, ensure_ascii=False) + "\n")

    for name, (w, h, fn) in IMAGES.items():
        write_png(IMG / name, w, h, fn)

    print(f"wrote {len(posts)} posts, {len(comments)} comment trees, {len(SUBREDDITS)} subreddits, {len(IMAGES)} images")

if __name__ == "__main__":
    main()
