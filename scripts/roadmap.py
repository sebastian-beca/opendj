#!/usr/bin/env python3
"""Roadmap status, worked out from each OpenSpec change's own folder.

openspec/roadmap.md only says which release each change is planned for and
in what order. How far a change has got is not written there: it comes from
the ticked steps in openspec/changes/<name>/tasks.md, so a pull request that
moves a change forward edits only that change's folder and two open pull
requests don't collide on one shared line.

    scripts/roadmap.py            print every listed change with its status
    scripts/roadmap.py v0.6       only the releases whose heading starts so
    scripts/roadmap.py --check    fail if a change folder is missing from the
                                  roadmap, listed twice in one release, or a
                                  link is broken
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "openspec"
ROADMAP = ROOT / "roadmap.md"
CHANGES = ROOT / "changes"

LINK = re.compile(r"\(changes/([^/)]+)/proposal\.md\)")
TASK = re.compile(r"^\s*- \[([ xX])\]", re.MULTILINE)


def sections():
    """(heading, [change names]) in the order the roadmap lists them."""
    out = []
    heading = ""
    for line in ROADMAP.read_text(encoding="utf-8").splitlines():
        if line.startswith("## "):
            heading = line[3:].strip()
            out.append((heading, []))
        elif line.lstrip().startswith("- ") and out:
            out[-1][1].extend(LINK.findall(line))
    return out


def change_folders():
    return sorted(
        p.name for p in CHANGES.iterdir() if p.is_dir() and p.name != "archive"
    )


def status(name):
    tasks = CHANGES / name / "tasks.md"
    if not tasks.exists():
        return 0, 0, "no tasks"
    marks = TASK.findall(tasks.read_text(encoding="utf-8"))
    done = sum(1 for m in marks if m in "xX")
    total = len(marks)
    if total == 0:
        word = "no tasks"
    elif done == 0:
        word = "not started"
    elif done == total:
        word = "built"
    else:
        word = "in progress"
    return done, total, word


def check():
    problems = []
    listed = {}
    for heading, names in sections():
        for n in names:
            listed.setdefault(n, []).append(heading)
    for n, where in sorted(listed.items()):
        # A change can have steps in more than one release, but two lines
        # for it in one release usually mean a merge kept both versions.
        twice = sorted({h for h in where if where.count(h) > 1})
        if twice:
            problems.append(f"{n} is listed more than once in {', '.join(twice)}")
        if not (CHANGES / n / "proposal.md").exists():
            problems.append(f"{n} is listed but has no changes/{n}/proposal.md")
    for n in change_folders():
        if n not in listed:
            problems.append(f"{n} has a folder but is not in roadmap.md")
    for p in problems:
        print(f"roadmap: {p}", file=sys.stderr)
    return 1 if problems else 0


def show(prefix):
    for heading, names in sections():
        if not names or (prefix and not heading.startswith(prefix)):
            continue
        print(f"\n{heading}")
        width = max(len(n) for n in names)
        for n in names:
            done, total, word = status(n)
            count = f"{done}/{total}" if total else ""
            print(f"  {n:<{width}}  {count:>7}  {word}")


def main(args):
    if "--check" in args:
        return check()
    show(args[0] if args else "")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
