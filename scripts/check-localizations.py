#!/usr/bin/env python3
"""Validate paired ARB messages and compare official Flutter-generated output."""
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / 'apps/flutter'
ARB = APP / 'lib/l10n'

def main():
    catalogs = {language: json.loads((ARB / f'app_{language}.arb').read_text())
                for language in ('en', 'zh')}
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
    # Generate in a disposable project, leaving the checkout untouched.
    with tempfile.TemporaryDirectory(prefix='anywhere-l10n-') as temporary:
        project = Path(temporary)
        (project / 'pubspec.yaml').write_text((APP / 'pubspec.yaml').read_text())
        configuration = {
            'arb-dir': str(ARB), 'output-dir': str(project / 'generated'),
            'template-arb-file': 'app_en.arb',
            'output-localization-file': 'app_localizations.dart',
            'nullable-getter': False, 'format': False,
        }
        (project / 'l10n.yaml').write_text(json.dumps(configuration))
        subprocess.run(['flutter', '--no-version-check', 'gen-l10n', '--project-dir', temporary],
                       cwd=project, check=True)
        generated = {p.name: p.read_bytes() for p in (project / 'generated').glob('*.dart')}
        committed = {p.name: p.read_bytes() for p in (ARB / 'generated').glob('*.dart')}
        if not generated or generated != committed:
            raise SystemExit('Generated localizations are stale; run flutter gen-l10n')
    print(f'Localizations OK: {len(keys(catalogs["en"]))} messages, en/zh, official generated output')

if __name__ == '__main__':
    main()
