#!/usr/bin/env python3
"""Build the isolated P0 probe using the app's existing assemble/swiftc model.

No CocoaPods installation, production dependency changes, or user-data access.
The only extra native dependency is the plugin's pinned OrderedSet pod source.
"""
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import webview_native
from webview_native import ROOT, run
PROBE = ROOT / 'experiments/web_plugin_probe'
WORK = ROOT / 'build/web-plugin-probe'
APP = ROOT / 'build/WebPluginProbe.app'


def main():
    if sys.platform != 'darwin':
        raise SystemExit('P0 native packaging currently requires macOS.')
    run(['python3', 'scripts/check-flutter.py'])
    identity = os.environ.get('ANYWHERE_CODESIGN_IDENTITY', 'Anywhere Local Code Signing')
    identities = subprocess.check_output(['security', 'find-identity', '-p', 'codesigning'], text=True)
    if identity == '-' or identity not in identities:
        raise SystemExit('A persistent code signing identity is required; see scripts/setup-local-signing.sh.')
    WORK.mkdir(parents=True, exist_ok=True)
    run(['flutter', '--no-version-check', 'pub', 'get', '--enforce-lockfile'], PROBE)
    arch = subprocess.check_output(['uname', '-m'], text=True).strip()
    flutter_output = WORK / 'flutter'
    run(['flutter', '--no-version-check', 'assemble', f'--output={flutter_output}',
         '-dTargetPlatform=darwin', f'-dDarwinArchs={arch}', '-dBuildMode=release',
         '-dTargetFile=lib/main.dart', 'release_macos_bundle_flutter_assets'], PROBE)

    import webview_native
    framework, plugin, ordered = webview_native.build_framework(flutter_output, arch, PROBE)
    flutter_framework = flutter_output / 'FlutterMacOS.framework'
    native = framework.parent

    if APP.exists():
        shutil.rmtree(APP)
    frameworks = APP / 'Contents/Frameworks'
    resources = APP / 'Contents/Resources'
    executable = APP / 'Contents/MacOS/WebPluginProbe'
    frameworks.mkdir(parents=True)
    resources.mkdir()
    executable.parent.mkdir()
    for source in [flutter_framework, flutter_output / 'App.framework', framework]:
        shutil.copytree(source, frameworks / source.name, symlinks=True)
    licenses = resources / 'Licenses'
    licenses.mkdir()
    shutil.copy2(plugin / 'LICENSE', licenses / 'flutter_inappwebview-LICENSE')
    shutil.copy2(ordered / 'LICENSE', licenses / 'OrderedSet-LICENSE')
    shutil.copy2(ordered / 'Framework/PrivacyInfo.xcprivacy', resources / 'OrderedSet-PrivacyInfo.xcprivacy')
    info = {'CFBundleIdentifier': 'dev.anywhere.WebPluginProbe',
            'CFBundleExecutable': 'WebPluginProbe', 'CFBundleName': 'WebPluginProbe',
            'CFBundlePackageType': 'APPL', 'CFBundleVersion': '1',
            'CFBundleShortVersionString': '0.0.1', 'LSMinimumSystemVersion': '12.0',
            'NSPrincipalClass': 'NSApplication', 'NSHighResolutionCapable': True,
            'FLTEnableImpeller': True}
    (APP / 'Contents/Info.plist').write_bytes(plistlib.dumps(info))
    run(['xcrun', 'swiftc', '-O', '-target', f'{arch}-apple-macos12.0',
         '-F', frameworks, '-I', native, '-L', native,
         '-framework', 'FlutterMacOS', '-framework', 'flutter_inappwebview_macos',
         '-Xlinker', '-rpath', '-Xlinker', '@executable_path/../Frameworks',
         PROBE / 'macos/Probe.swift', '-o', executable])
    for name in ['FlutterMacOS', 'App', 'flutter_inappwebview_macos']:
        run(['codesign', '--force', '--sign', identity, frameworks / f'{name}.framework'])
    run(['codesign', '--force', '--sign', identity, APP])
    run(['codesign', '--verify', '--deep', '--strict', APP])
    print(f'Built {APP}; synthetic-only P0 probe, not the production launcher.')


if __name__ == '__main__':
    main()
