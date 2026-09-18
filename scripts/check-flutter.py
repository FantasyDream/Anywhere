#!/usr/bin/env python3
"""Require the Flutter release declared in .flutter-version to be on PATH."""
import json
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
expected = (root / '.flutter-version').read_text().strip()
flutter = shutil.which('flutter')
if flutter is None:
    sys.exit(f'Flutter {expected} is required on PATH.')
result = subprocess.run([flutter, '--version', '--machine'], text=True,
                        capture_output=True, check=True)
metadata = json.loads(result.stdout)
actual = metadata['frameworkVersion']
print(f"Flutter {actual}; framework={metadata['frameworkRevision']}; "
      f"engine={metadata['engineRevision']}; Dart={metadata['dartSdkVersion']}")
if actual != expected:
    sys.exit(f'Flutter SDK mismatch: expected {expected}, got {actual}. '
             'Install that official stable release and keep it on PATH.')
