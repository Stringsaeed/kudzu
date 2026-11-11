#!/usr/bin/env python3
"""Audit S20's copied tests and fixtures against a pre-move git revision.

Run from any directory: python3 crates/kudzu-core/scripts/check-restructure.py REV
This compares Rust tokens, not whitespace or comments. Only the listed relocated
paths and type rename are normalized; literals and assertion operators are exact.
Cargo test separately proves that these source tests are compiled and executed.
"""

import collections
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[5]
CRATES = Path(__file__).resolve().parents[2]
TOKEN = re.compile(
    r'//[^\n]*|/\*.*?\*/|(?:br|r)(\#*)".*?"\1|b?"(?:\\.|[^"\\])*"'
    r"|b?'(?:\\.|[^'\\\n])'|[a-zA-Z_][a-zA-Z_0-9]*|\d[\w.]*|[^\s]",
    re.S,
)


def git(*args):
    return subprocess.check_output(["git", "-C", str(ROOT), *args])


def tokens(source):
    return [
        m.group() for m in TOKEN.finditer(source)
        if not m.group().startswith(("//", "/*"))
    ]


PATHS = [
    ("kudzu_core::interchange::dedupe", "kudzu_core::library::dedupe"),
    ("crate::interchange::stats", "crate::library::usage"),
    ("coordinator::ensure_getting_started_snippets", "starter::ensure_getting_started_snippets"),
    ("coordinator::merge_library", "merge::merge_library"),
    ("coordinator::ImportSource", "detect::ImportSource"),
    ("crate::key", "self::key"),
    ("Skip" + "Cause", "SkipReason"),
]


def canonical(body):
    for old, new in PATHS:
        before, after = tokens(old), tokens(new)
        i = 0
        while i <= len(body) - len(before):
            if body[i:i + len(before)] == before:
                body[i:i + len(before)] = after
                i += len(after)
            else:
                i += 1
    return tuple(body)


def tests(source):
    ts = tokens(source)
    for i in range(len(ts) - 6):
        if ts[i:i + 5] != ["#", "[", "test", "]", "fn"]:
            continue
        name = ts[i + 5]
        start = ts.index("{", i + 6)
        depth = 1
        end = start + 1
        while depth:
            depth += (ts[end] == "{") - (ts[end] == "}")
            end += 1
        yield name, canonical(ts[start:end])


def inventory(files):
    found = collections.defaultdict(collections.Counter)
    for path, source in files:
        for name, body in tests(source):
            found[name][body] += 1
    return found


def main():
    revision, = sys.argv[1:]
    for crate in ("kudzu-core", "kudzu-engine"):
        directory = CRATES / crate
        relative = directory.relative_to(ROOT).as_posix()
        paths = git("ls-tree", "-r", "--name-only", revision, "--", relative).decode().splitlines()
        old = inventory((p, git("show", f"{revision}:{p}").decode()) for p in paths if p.endswith(".rs"))
        new = inventory((p, p.read_text()) for p in directory.rglob("*.rs"))
        missing = {name for name in old if old[name] != new.get(name)}
        if missing:
            raise SystemExit(f"{crate}: changed or missing copied test bodies: {sorted(missing)}")
        count = sum(sum(bodies.values()) for bodies in old.values())
        additions = sorted(set(new) - set(old))
        print(f"{crate}: {count} copied test names and bodies unchanged; added {additions}")
        for p in paths:
            if "/tests/fixtures/" in p:
                assert (ROOT / p).read_bytes() == git("show", f"{revision}:{p}"), p
        print(f"{crate}: every existing fixture byte unchanged")

    core = CRATES / "kudzu-core"
    longest = (0, None)
    for path in (core / "src").rglob("*.rs"):
        if path.name == "tests.rs":
            continue
        # Unit suites are separate files; inline test modules end production code.
        source = path.read_text().split("#[cfg(test)]\nmod tests", 1)[0]
        lines = len(source.splitlines())
        assert lines <= 600, f"{path}: {lines} production lines"
        assert path.name not in ("utils.rs", "helpers.rs", "common.rs"), path
        if lines > longest[0]:
            longest = (lines, path.relative_to(core))
    print(f"core source limit: longest file {longest[1]} ({longest[0]} lines)")


if __name__ == "__main__":
    main()
