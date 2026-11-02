#!/usr/bin/env python3
"""Record the immutable source and development-signing boundary of a CI package."""
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys

app = Path(sys.argv[1]).resolve()
info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'],
                                 text=True).strip()
if os.environ.get('GITHUB_SHA') != commit:
    sys.exit('The checkout does not match the workflow source commit.')
result = {
    'version':
        info['CFBundleShortVersionString'],
    'build':
        info['CFBundleVersion'],
    'commit':
        commit,
    'architecture':
        subprocess.check_output(['uname', '-m'], text=True).strip(),
    'signing':
        'Anywhere CI Development (self-signed; not notarized)',
    'workflow':
        f"{os.environ['GITHUB_SERVER_URL']}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}",
}
(app.parent /
 'package-provenance.json').write_text(json.dumps(result, indent=2) + '\n')
with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
    summary.write(
        f"## macOS development package\n\nVersion: {result['version']}+{result['build']}\n\n"
    )
    summary.write(
        f"Source: `{commit}`\n\nArchitecture: `{result['architecture']}`\n\n")
    summary.write(
        'ZIP, SHA-256 and source metadata are available in this run’s artifact.\n\n'
    )
    summary.write(
        'Signed with the persistent CI development identity; not Developer ID notarized.\n'
    )
