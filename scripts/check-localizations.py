#!/usr/bin/env python3
"""Validate the paired GPUI and native ARB messages."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ARB = ROOT / 'resources/localization'


def main():
    catalogs = {
        language: json.loads((ARB / f'app_{language}.arb').read_text())
        for language in ('en', 'zh')
    }
    keys = lambda values: {key for key in values if not key.startswith('@')}
    if keys(catalogs['en']) != keys(catalogs['zh']):
        raise SystemExit('English and Chinese message keys differ')
    for key in keys(catalogs['en']):
        for language, values in catalogs.items():
            if not isinstance(values[key], str) or not values[key].strip():
                raise SystemExit(f'Empty localization: {language}/{key}')
        en = catalogs['en'].get('@' + key, {}).get('placeholders', {})
        zh = catalogs['zh'].get('@' + key, {}).get('placeholders', {})
        if en != zh:
            raise SystemExit(f'Placeholder definitions differ: {key}')
    print(f'Localizations OK: {len(keys(catalogs["en"]))} messages, en/zh')


if __name__ == '__main__':
    main()
