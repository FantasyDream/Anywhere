#!/usr/bin/env python3
"""Package local Web assets; the Anywhere Host performs authoritative validation."""
import argparse
import json
import os
from pathlib import Path
import shutil
import tempfile
import zipfile


def runtime_paths(root):
    yield root
    if root.is_dir():
        for directory, dirs, names in os.walk(root, followlinks=False):
            for name in [*dirs, *names]:
                yield Path(directory) / name


def package(project, destination, assets=("dist",)):
    project, destination = Path(project).resolve(), Path(destination).resolve()
    if destination.suffix != ".awplugin":
        raise ValueError("output must end with .awplugin")
    roots = ["manifest.json", *assets]
    files = set()
    total_bytes = 0
    for relative in roots:
        if not relative or Path(relative).is_absolute() or any(
                part in (".", "..") for part in relative.split("/")):
            raise ValueError("asset paths must be relative to the project")
        root = project / relative
        if not root.exists() or root.is_symlink():
            raise ValueError(f"missing asset or symbolic link: {relative}")
        if root.is_dir() and destination.is_relative_to(root.resolve()):
            raise ValueError("output must be outside the asset roots")
        # Never include source/dependencies implicitly; only explicit runtime roots.
        for path in runtime_paths(root):
            if path.is_symlink() or project not in path.resolve().parents:
                raise ValueError(
                    "assets must be regular project files, without symbolic links"
                )
            if path.is_file(
            ) and path.name != ".DS_Store" and path not in files:
                if path == destination:
                    raise ValueError("output must be outside the asset roots")
                size = path.stat().st_size
                total_bytes += size
                # Development-tool guards, not a second Host validator. Reject
                # while walking, before retaining an unbounded build directory.
                if len(files) >= 2000 or size > 16 * 1024 * 1024:
                    raise ValueError("too many or oversized runtime assets")
                if total_bytes > 20 * 1024 * 1024:
                    raise ValueError(
                        "runtime assets exceed this packer's 20 MiB stored ZIP budget"
                    )
                files.add(path)
    manifest = json.loads(
        (project / "manifest.json").read_text(encoding="utf-8"))
    if not isinstance(manifest, dict):
        raise ValueError("manifest must be a JSON object")
    entry = manifest.get("entry", "")
    names = {path.relative_to(project).as_posix() for path in files}
    if not isinstance(entry, str) or (entry and entry not in names):
        raise ValueError("manifest entry is missing from the packaged assets")
    background = manifest.get("background")
    if background is not None and (not isinstance(background, dict) or
                                   background.get("entry") not in names):
        raise ValueError("background entry is missing from the packaged assets")
    if not entry and background is None:
        raise ValueError("manifest needs a UI or background entry")
    icon = manifest.get("icon")
    if icon is not None and (not isinstance(icon, str) or icon not in names):
        raise ValueError("manifest icon is missing; include it in --assets")
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination.parent,
                                         delete=False) as work:
            temporary = Path(work.name)
        with zipfile.ZipFile(temporary, "w",
                             compression=zipfile.ZIP_STORED) as archive:
            for path in sorted(
                    files,
                    key=lambda path: path.relative_to(project).as_posix()):
                item = zipfile.ZipInfo(
                    path.relative_to(project).as_posix(), (2026, 1, 1, 0, 0, 0))
                item.external_attr = 0o100644 << 16
                with path.open("rb") as source, archive.open(item,
                                                             "w") as target:
                    shutil.copyfileobj(source, target, length=64 * 1024)
        if temporary.stat().st_size > 20 * 1024 * 1024:
            raise ValueError("package exceeds 20 MiB")
        temporary.replace(destination)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("project",
                        type=Path,
                        help="directory containing manifest.json")
    parser.add_argument("--output", "-o", required=True, type=Path)
    parser.add_argument(
        "--assets",
        action="append",
        help="runtime file/directory; repeatable, default: dist")
    args = parser.parse_args()
    try:
        result = package(args.project, args.output, args.assets or ["dist"])
    except (ValueError, OSError, zipfile.BadZipFile) as error:
        parser.exit(1, f"Cannot package plugin: {error}\n")
    print(
        f"Built {result} ({result.stat().st_size:,} bytes); import into Anywhere to validate and approve."
    )


if __name__ == "__main__":
    main()
