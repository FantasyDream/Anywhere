#!/usr/bin/env python3
"""Package the Windows x64 development preview and write its SHA-256."""
import argparse
import hashlib
from pathlib import Path
import struct
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def require_windows_x64(executable):
    with executable.open('rb') as binary:
        header = binary.read(64)
        if len(header) == 64 and header[:2] == b'MZ':
            offset, = struct.unpack_from('<I', header, 60)
            binary.seek(offset)
            if binary.read(6) == b'PE\0\0\x64\x86':
                return
    raise ValueError('expected a Windows x64 Portable Executable')


def package(executable):
    require_windows_x64(executable)
    output = ROOT / 'build' / 'windows'
    if output.resolve() != output:
        raise ValueError('Windows package output must stay under build/windows')
    output.mkdir(parents=True, exist_ok=True)
    archive_name = 'Anywhere-windows-x64.zip'
    checksum_name = archive_name + '.sha256'

    with tempfile.TemporaryDirectory(dir=output) as temp:
        staging = Path(temp)
        archive = staging / archive_name
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as bundle:
            bundle.write(executable, 'Anywhere/Anywhere.exe')
            bundle.write(ROOT / 'LICENSE', 'Anywhere/LICENSE')
            bundle.write(ROOT / 'docs' / 'windows.md', 'Anywhere/README.md')
            for notice in sorted(
                (ROOT / 'resources' / 'licenses').glob('*.txt')):
                bundle.write(notice, f'Anywhere/Licenses/{notice.name}')
        with zipfile.ZipFile(archive) as bundle:
            if bundle.testzip() is not None:
                raise ValueError('Windows archive failed its integrity check')

        digest = hashlib.sha256()
        with archive.open('rb') as source:
            for block in iter(lambda: source.read(1024 * 1024), b''):
                digest.update(block)
        checksum = staging / checksum_name
        checksum.write_text(f'{digest.hexdigest()}  {archive_name}\n',
                            encoding='ascii')
        archive.replace(output / archive_name)
        checksum.replace(output / checksum_name)
    return output / archive_name


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    args = parser.parse_args()
    try:
        print(package(args.executable))
    except (OSError, ValueError) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
