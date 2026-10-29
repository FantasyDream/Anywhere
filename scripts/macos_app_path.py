#!/usr/bin/env python3
"""Resolve a selected macOS bundle without allowing build-directory escapes."""
from pathlib import Path
import sys


def app_path(root: Path, value: str) -> Path:
    requested = Path(value)
    if not requested.is_absolute():
        requested = root / requested
    # Resolving a leaf link before rm -rf would delete its target instead of
    # replacing only the selected link. Require an ordinary bundle location.
    if requested.is_symlink():
        raise ValueError('The selected app must not be a symbolic link.')
    destination = requested.resolve()
    try:
        destination.relative_to(root.resolve() / 'build')
    except ValueError:
        raise ValueError(
            'Select an .app path inside this checkout\u2019s build directory.'
        ) from None
    if destination.suffix != '.app':
        raise ValueError('The selected output must have an .app suffix.')
    return destination


if __name__ == '__main__':
    if len(sys.argv) != 2:
        sys.exit('Usage: scripts/macos_app_path.py <app path>')
    try:
        print(app_path(Path(__file__).resolve().parent.parent, sys.argv[1]))
    except (ValueError, OSError, RuntimeError) as error:
        sys.exit(str(error))
