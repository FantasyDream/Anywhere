#!/usr/bin/env python3
"""Check repository Markdown targets and heading links without network access."""
from collections import Counter
import argparse
from pathlib import Path
import re
import subprocess
import unicodedata
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent


def repository_documents(root):
    # Check the files that a checkout / PR will actually ship, including new
    # root community files, SDK guides and example READMEs. Ignore build caches.
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "*.md"],
        cwd=root, check=True, capture_output=True,
    )
    return sorted({root / name.decode("utf-8") for name in result.stdout.split(b"\0")
                   if name and (root / name.decode("utf-8")).is_file()})


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


def check_documents(root, documents, *, check_ignored=True):
    failures, ignored, headings = [], {}, {}
    links = 0
    for document in documents:
        text = document.read_text()
        source = document.relative_to(root)
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
                relative = target.relative_to(root)
            except ValueError:
                failures.append(f"{source}: {raw}: target outside repository")
                continue
            if check_ignored and relative not in ignored:
                ignored[relative] = subprocess.run(
                    ["git", "check-ignore", "-q", str(relative)], cwd=root,
                    check=False,
                ).returncode == 0
            if ignored.get(relative, False):
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
    return failures, links


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=Path,
                        help='Check an extracted SDK without Git or repository community files')
    package = parser.parse_args().package
    if package is not None:
        root = package.resolve()
        if not root.is_dir():
            raise SystemExit(f'Package directory does not exist: {root}')
        documents = sorted(root.rglob('*.md'))
        if not documents:
            raise SystemExit('Package contains no Markdown documentation')
        failures, links = check_documents(root, documents, check_ignored=False)
        if failures:
            raise SystemExit('\n'.join(failures))
        print(f'Package documentation OK: {len(documents)} Markdown files, {links} local links')
        return
    documents = repository_documents(ROOT)
    failures, links = check_documents(ROOT, documents)
    for name in ["LICENSE", "CONTRIBUTING.md", "CODE_OF_CONDUCT.md", "SECURITY.md",
                 "SUPPORT.md", "CHANGELOG.md", ".github/PULL_REQUEST_TEMPLATE.md",
                 ".github/ISSUE_TEMPLATE/bug_report.yml",
                 ".github/ISSUE_TEMPLATE/feature_request.yml"]:
        if not (ROOT / name).is_file():
            failures.append(f"Missing project file: {name}")
    sdk_license = ROOT / "packages/plugin-sdk/LICENSE"
    project_license = ROOT / "LICENSE"
    if (not project_license.is_file() or not sdk_license.is_file()
            or sdk_license.read_bytes() != project_license.read_bytes()):
        failures.append("Plugin SDK license must match the repository MIT license")
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"Documentation OK: {len(documents)} Markdown files, {links} local links; project files and licenses present")


if __name__ == "__main__":
    main()
