#!/usr/bin/env python3
"""Check repository Markdown targets and heading links without network access."""
from collections import Counter
from pathlib import Path
import re
import subprocess
import unicodedata
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent


def heading_ids(text):
    result, counts, fenced = set(), Counter(), False
    for line in text.splitlines():
        if line.startswith("```"):
            fenced = not fenced
            continue
        if fenced or not re.match(r"^#{1,6}\s", line):
            continue
        heading = re.sub(r"^#{1,6}\s+", "", line).strip().lower()
        heading = re.sub(r"\[([^]]+)\]\([^)]*\)", r"\1", heading)
        heading = "".join(
            c for c in heading
            if c in "-_" or c.isspace() or unicodedata.category(c)[0] in "LN"
        ).replace(" ", "-")
        count = counts[heading]
        counts[heading] += 1
        result.add(heading + (f"-{count}" if count else ""))
    return result


def main():
    documents = [ROOT / "README.md", *sorted((ROOT / "docs").rglob("*.md"))]
    failures, ignored, headings = [], {}, {}
    links = 0
    for document in documents:
        text = document.read_text()
        source = document.relative_to(ROOT)
        if text.count("```") % 2:
            failures.append(f"{source}: unclosed code fence")
        for raw in re.findall(r"\]\(([^)\n]+)\)", text):
            raw = raw.strip().strip("<>")
            if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", raw):
                continue
            path, _, anchor = raw.partition("#")
            target = (document.parent / unquote(path)).resolve() if path else document
            links += 1
            try:
                relative = target.relative_to(ROOT)
            except ValueError:
                failures.append(f"{source}: {raw}: target outside repository")
                continue
            if relative not in ignored:
                ignored[relative] = subprocess.run(
                    ["git", "check-ignore", "-q", str(relative)], cwd=ROOT,
                    check=False,
                ).returncode == 0
            if ignored[relative]:
                failures.append(f"{source}: {raw}: ignored target is not shipped")
                continue
            if not target.exists():
                failures.append(f"{source}: {raw}: missing target")
                continue
            if anchor and target.suffix == ".md":
                if target not in headings:
                    headings[target] = heading_ids(target.read_text())
                if unquote(anchor) not in headings[target]:
                    failures.append(f"{source}: {raw}: missing heading")
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"Documentation OK: {len(documents)} Markdown files, {links} local links; no ignored targets")


if __name__ == "__main__":
    main()
