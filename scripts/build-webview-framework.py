#!/usr/bin/env python3
"""Package the pinned native WebView using the same integration as the P0 probe."""
import shutil
import subprocess
import sys
from pathlib import Path
from webview_native import ROOT, build_framework
app = Path(sys.argv[1])
arch = subprocess.check_output(['uname', '-m'], text=True).strip()
framework, plugin, ordered = build_framework(ROOT / 'build/flutter', arch)
subprocess.run(['ditto', str(framework), str(app / 'Contents/Frameworks' / framework.name)], check=True)
licenses = app / 'Contents/Resources/Licenses'
licenses.mkdir(exist_ok=True)
shutil.copy2(plugin / 'LICENSE', licenses / 'flutter_inappwebview.txt')
shutil.copy2(ordered / 'LICENSE', licenses / 'OrderedSet.txt')
shutil.copy2(ordered / 'Framework/PrivacyInfo.xcprivacy', app / 'Contents/Resources/OrderedSet-PrivacyInfo.xcprivacy')
