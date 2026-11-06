#!/usr/bin/env python3
"""Converts mpump's drum patterns into the Groove Box's grooves format.

mpump (https://github.com/gdamdam/mpump, by gdamdam) keeps its drum
patterns in server/public/data/patterns-t8-drums.json and their names in
server/public/data/catalog.json. This writes them as
crates/engine/data/grooves/mpump.txt (see basics.txt there for the format).

Usage: import-mpump-grooves.py <mpump checkout> <output file>
"""

import json
import sys
from pathlib import Path

# General MIDI drum notes to the Groove Box's sounds.
ROWS = {
    36: "kick",
    38: "snare",
    42: "closed-hat",
    46: "open-hat",
    50: "clap",
    37: "rim",
    47: "cowbell",
    56: "cowbell",
    49: "crash",
    51: "ride",
}
ORDER = ["kick", "snare", "closed-hat", "open-hat", "clap", "rim", "cowbell", "crash", "ride"]

GENRE_NAMES = {
    "idm": "IDM",
    "edm": "EDM",
    "drum-and-bass": "Drum & Bass",
    "lo-fi": "Lo-Fi",
}


def genre_name(key):
    return GENRE_NAMES.get(key) or " ".join(w.capitalize() for w in key.split("-"))


def step(vel):
    if vel < 75:
        return "o"
    if vel >= 110:
        return "X"
    return "x"


def main():
    src = Path(sys.argv[1]) / "server" / "public" / "data"
    patterns = json.loads((src / "patterns-t8-drums.json").read_text())
    catalog = json.loads((src / "catalog.json").read_text())["t8"]["drum_genres"]
    out = [
        "# SPDX-License-Identifier: AGPL-3.0-only",
        "# Drum patterns from mpump, Copyright gdamdam (https://github.com/gdamdam/mpump),",
        "# under the GNU Affero General Public License, version 3. Converted from its",
        "# server/public/data/patterns-t8-drums.json and catalog.json by",
        "# scripts/import-mpump-grooves.py. See NOTICE.",
        "",
    ]
    for genre in catalog:
        out.append(f"genre {genre_name(genre['name'])}")
        out.append("")
        for info, steps in zip(genre["patterns"], patterns[genre["name"]]):
            rows = {name: ["."] * 16 for name in ORDER}
            for i, hits in enumerate(steps[:16]):
                for hit in hits or []:
                    row = ROWS.get(hit["note"])
                    if row is None:
                        continue
                    mark = step(hit["vel"])
                    # Two notes on one row (both cowbells): keep the louder.
                    if rows[row][i] == "." or mark == "X" or (mark == "x" and rows[row][i] == "o"):
                        rows[row][i] = mark
            about = info.get("desc", "").replace("—", "-").replace("|", "/")
            out.append(f"groove {info['name']} | {about}")
            for name in ORDER:
                if any(c != "." for c in rows[name]):
                    out.append(f"{name:<10} {''.join(rows[name])}")
            out.append("")
    Path(sys.argv[2]).write_text("\n".join(out))


if __name__ == "__main__":
    main()
