#!/usr/bin/env python3
"""Shared pinned WebKit plugin integration for the manual macOS build hosts."""
import hashlib
import io
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import urllib.parse
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
WORK = ROOT / 'build/webview-native'
ORDERED_URL = 'https://codeload.github.com/Weebly/OrderedSet/zip/refs/tags/v6.0.3'
ORDERED_SHA = 'b7fcd9a8980fb35715e5c6c0ebe66209f0f44c57f823b2c6efca8eb8c3b6f193'


def run(args, cwd=ROOT):
    subprocess.run([str(arg) for arg in args], cwd=cwd, check=True)


def package_root(name, project):
    config = project / '.dart_tool/package_config.json'
    package = next(p for p in json.loads(config.read_text())['packages']
                   if p['name'] == name)
    uri = urllib.parse.urlparse(
        urllib.parse.urljoin(config.as_uri(), package['rootUri']))
    if uri.scheme != 'file' or uri.netloc:
        raise ValueError('Expected a local pub package')
    return Path(urllib.parse.unquote(uri.path))


def ordered_set_source():
    archive = WORK / 'dependencies/OrderedSet-6.0.3.zip'
    archive.parent.mkdir(parents=True, exist_ok=True)
    if archive.exists():
        data = archive.read_bytes()
    else:
        with urllib.request.urlopen(ORDERED_URL, timeout=30) as response:
            data = response.read(1024 * 1024)
    if hashlib.sha256(data).hexdigest() != ORDERED_SHA:
        raise ValueError('OrderedSet archive checksum mismatch')
    archive.write_bytes(data)
    destination = archive.parent / 'source'
    destination.mkdir(exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(data)) as source:
        for entry in source.infolist():
            path = Path(entry.filename)
            if path.is_absolute(
            ) or '..' in path.parts or not path.parts or path.parts[
                    0] != 'OrderedSet-6.0.3':
                raise ValueError('Invalid dependency archive path')
            if (entry.external_attr >> 16) & 0o170000 == 0o120000:
                raise ValueError('Dependency symlinks are not permitted')
            if entry.file_size > 1024 * 1024:
                raise ValueError('Dependency file exceeds limit')
        source.extractall(destination)
    return destination / 'OrderedSet-6.0.3'


def framework_layout(path, name):
    version = path / 'Versions/A'
    (version / 'Resources').mkdir(parents=True)
    (version / 'Modules').mkdir()
    (version / 'Headers').mkdir()
    (path / 'Versions/Current').symlink_to('A')
    for part in [name, 'Resources', 'Modules', 'Headers']:
        (path / part).symlink_to(f'Versions/Current/{part}')
    # CocoaPods normally supplies this umbrella module. Several upstream Swift
    # files rely on its Cocoa import; reproduce it without rewriting pub sources.
    (version / 'Headers' / f'{name}.h').write_text('#import <Cocoa/Cocoa.h>\n')
    (version / 'Modules/module.modulemap').write_text(
        f'framework module {name} {{\n  umbrella header "{name}.h"\n  export *\n}}\n'
    )
    info = {
        'CFBundleIdentifier': f'dev.anywhere.frameworks.{name}',
        'CFBundleExecutable': name,
        'CFBundlePackageType': 'FMWK',
        'CFBundleVersion': '1',
        'CFBundleShortVersionString': '1.1.2'
    }
    (version / 'Resources/Info.plist').write_bytes(plistlib.dumps(info))
    return version


def build_framework(flutter_output, arch, project=ROOT / 'apps/flutter'):
    flutter_framework = flutter_output / 'FlutterMacOS.framework'
    plugin = package_root('flutter_inappwebview_macos', project)
    if 'version: 1.1.2\n' not in (plugin / 'pubspec.yaml').read_text():
        raise ValueError('Unexpected macOS WebView implementation version')
    ordered = ordered_set_source()
    sources = sorted((plugin / 'macos/Classes').rglob('*.swift'))
    ordered_sources = sorted((ordered / 'Sources').rglob('*.swift'))
    native = WORK / 'native'
    native.mkdir(exist_ok=True)
    os.environ.setdefault('CLANG_MODULE_CACHE_PATH',
                          str(native / 'clang-cache'))
    os.environ.setdefault('SWIFT_MODULE_CACHE_PATH',
                          str(native / 'swift-cache'))
    framework = native / 'flutter_inappwebview_macos.framework'
    digest = hashlib.sha256()
    digest.update((str(flutter_framework) + arch + ORDERED_SHA).encode())
    # The engine path is stable across SDK upgrades; its pinned version is part
    # of the native ABI input, so a new Flutter release must rebuild this cache.
    digest.update((ROOT / '.flutter-version').read_bytes())
    digest.update(subprocess.check_output(['xcrun', 'swiftc', '--version']))
    for path in [Path(__file__), *sources, *ordered_sources]:
        digest.update(str(path).encode())
        digest.update(path.read_bytes())
    stamp = native / 'source.sha256'
    if not framework.exists() or not stamp.exists() or stamp.read_text(
    ) != digest.hexdigest():
        stamp.unlink(missing_ok=True)
        if framework.exists():
            shutil.rmtree(framework)
        version = framework_layout(framework, 'flutter_inappwebview_macos')
        target = f'{arch}-apple-macos12.0'
        run([
            'xcrun', 'swiftc', '-O', '-target', target, '-emit-library',
            '-static', '-emit-module', '-module-name', 'OrderedSet',
            '-emit-module-path', native / 'OrderedSet.swiftmodule',
            *ordered_sources, '-o', native / 'libOrderedSet.a'
        ])
        modules = version / 'Modules/flutter_inappwebview_macos.swiftmodule'
        modules.mkdir()
        run([
            'xcrun', 'swiftc', '-O', '-swift-version', '5', '-target', target,
            '-emit-library', '-emit-module', '-module-name',
            'flutter_inappwebview_macos', '-emit-module-path',
            modules / f'{arch}-apple-macos.swiftmodule', '-I', native, '-L',
            native, '-lOrderedSet', '-F', flutter_output, '-F', native,
            '-import-underlying-module', '-framework', 'FlutterMacOS', *sources,
            '-Xlinker', '-install_name', '-Xlinker',
            '@rpath/flutter_inappwebview_macos.framework/Versions/A/flutter_inappwebview_macos',
            '-o', version / 'flutter_inappwebview_macos'
        ])
        shutil.copy2(plugin / 'macos/Resources/PrivacyInfo.xcprivacy',
                     version / 'Resources/PrivacyInfo.xcprivacy')
        for storyboard in sorted(
            (plugin / 'macos/Storyboards').glob('*.storyboard')):
            run([
                'xcrun', 'ibtool', '--compile',
                version / 'Resources' / f'{storyboard.stem}.storyboardc',
                storyboard
            ])
        stamp.write_text(digest.hexdigest())

    return framework, plugin, ordered
