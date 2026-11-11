#!/usr/bin/env python3
"""Project the shared ARB catalog into Apple's native .strings files."""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def generate(check=False):
    keys = json.loads(
        (ROOT / 'resources/native-localization-keys.json').read_text())
    if len(keys) != len(set(keys)):
        raise SystemExit('Duplicate native localization key')
    catalogs = {
        locale:
            json.loads(
                (ROOT / f'resources/localization/app_{locale}.arb').read_text())
        for locale in ('en', 'zh')
    }
    outputs = {}
    for locale, apple in [('en', 'en'), ('zh', 'zh-Hans')]:
        lines = ['/* Generated from the shared ARB catalog. */']
        for key in keys:
            lines.append(
                f'{json.dumps(key)} = {json.dumps(catalogs[locale][key], ensure_ascii=False)};'
            )
        outputs[
            ROOT /
            f'resources/native-localizations/{apple}.lproj/Anywhere.strings'] = '\n'.join(
                lines) + '\n'
        outputs[
            ROOT /
            f'resources/native-localizations/{apple}.lproj/InfoPlist.strings'] = (
                '/* Generated from the shared ARB catalog. */\n' +
                '"NSAppleEventsUsageDescription" = ' +
                json.dumps(catalogs[locale]['automationUsage'],
                           ensure_ascii=False) + ';\n')
    for path, value in outputs.items():
        if check:
            if not path.exists() or path.read_text() != value:
                raise SystemExit(
                    f'Stale native localizations: {path.relative_to(ROOT)}')
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(value)
    print(f'Native localizations OK: {len(keys)} keys, en/zh, Apple .strings')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    generate(parser.parse_args().check)
