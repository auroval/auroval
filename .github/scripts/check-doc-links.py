#!/usr/bin/env python3
"""Check that relative links between the project's Markdown files resolve.

The design documents cross-reference each other heavily and are the whole state
of the project, so a broken relative link is a real defect rather than a typo.

A script rather than an inline `run:` block because the shell version put its
`fail=1` inside a pipeline subshell, which meant the exit status could never be
anything but zero: the check looked like it worked and could not fail.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# Markdown inline links: [text](target). The target is captured without any
# "#fragment", which is checked no further than the file it names.
LINK = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
SKIP_DIRS = {"target", ".git", "node_modules"}


def main() -> int:
    root = Path.cwd()
    problems: list[str] = []
    checked = 0

    for md in sorted(root.rglob("*.md")):
        if any(part in SKIP_DIRS for part in md.relative_to(root).parts):
            continue
        text = md.read_text(encoding="utf-8", errors="replace")
        for match in LINK.finditer(text):
            target = match.group(1)
            # External links and in-page anchors are not ours to verify.
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            path = target.split("#", 1)[0]
            if not path:
                continue
            checked += 1
            resolved = (md.parent / path).resolve()
            if not resolved.exists():
                line = text[: match.start()].count("\n") + 1
                rel = md.relative_to(root)
                problems.append(f"{rel}:{line}: broken link: {target}")
                print(f"::error file={rel},line={line}::broken link: {target}")

    print(f"checked {checked} relative link(s) across the repository")
    if problems:
        print(f"\n{len(problems)} broken link(s):", file=sys.stderr)
        for p in problems:
            print(f"  {p}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
