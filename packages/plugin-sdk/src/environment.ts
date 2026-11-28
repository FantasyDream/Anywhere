export type Language = 'en' | 'zh';
export type Platform = 'macos' | 'windows';

declare global {
  var __anywhereLanguage: unknown;
  var __anywherePlatform: unknown;
}

const supported = (value: unknown): Language => (value === 'zh' ? 'zh' : 'en');
const platform = (): Platform => {
  const host = globalThis.__anywherePlatform;
  if (host === 'macos' || host === 'windows') return host;
  return /Mac/i.test(globalThis.navigator?.platform ?? '')
    ? 'macos'
    : 'windows';
};

function shortcut(specification: string) {
  const parts = specification.split('+');
  const key = parts.pop();
  const raw = parts;
  if (
    !key ||
    new Set(raw).size !== raw.length ||
    raw.some(
      modifier => !['Mod', 'Meta', 'Ctrl', 'Alt', 'Shift'].includes(modifier),
    ) ||
    (raw.includes('Mod') && (raw.includes('Meta') || raw.includes('Ctrl')))
  ) {
    throw new TypeError(`Invalid shortcut: ${specification}`);
  }
  const modifiers = raw.map(modifier =>
    modifier === 'Mod' ? (platform() === 'macos' ? 'Meta' : 'Ctrl') : modifier,
  );
  return {key, modifiers};
}

/** Local presentation metadata and portable keyboard shortcuts. */
export const environment = {
  get language(): Language {
    return supported(globalThis.__anywhereLanguage);
  },
  get platform(): Platform {
    return platform();
  },
  onLanguageChange(listener: (language: Language) => void): () => void {
    const receive = (event: Event) =>
      listener(supported((event as CustomEvent).detail));
    globalThis.addEventListener('anywhere:language', receive);
    return () => globalThis.removeEventListener('anywhere:language', receive);
  },
  onPlatformChange(listener: (platform: Platform) => void): () => void {
    const receive = () => listener(platform());
    globalThis.addEventListener('anywhere:platform', receive);
    return () => globalThis.removeEventListener('anywhere:platform', receive);
  },
  formatShortcut(specification: string): string {
    const {key, modifiers} = shortcut(specification);
    if (platform() === 'windows') return [...modifiers, key].join('+');
    const glyphs = new Map([
      ['Meta', '⌘'],
      ['Ctrl', '⌃'],
      ['Alt', '⌥'],
      ['Shift', '⇧'],
      ['Enter', '↵'],
      ['Tab', '⇥'],
    ]);
    return [...modifiers, key].map(part => glyphs.get(part) ?? part).join('');
  },
  matchesShortcut(event: KeyboardEvent, specification: string): boolean {
    if (
      typeof event.key !== 'string' ||
      event.defaultPrevented ||
      event.repeat ||
      event.isComposing ||
      event.keyCode === 229 ||
      event.getModifierState?.('AltGraph')
    )
      return false;
    const {key, modifiers} = shortcut(specification);
    return (
      event.key.toLowerCase() === key.toLowerCase() &&
      event.metaKey === modifiers.includes('Meta') &&
      event.ctrlKey === modifiers.includes('Ctrl') &&
      event.altKey === modifiers.includes('Alt') &&
      event.shiftKey === modifiers.includes('Shift')
    );
  },
};
