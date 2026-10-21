export type Language = 'en' | 'zh';

declare global { var __anywhereLanguage: unknown; }
const supported = (value: unknown): Language => value === 'zh' ? 'zh' : 'en';

/** Read-only presentation metadata. It conveys no permission or native identity.
 * Existing packages need not adopt a translation library; resources stay local.
 */
export const environment = {
  get language(): Language { return supported(globalThis.__anywhereLanguage); },
  onLanguageChange(listener: (language: Language) => void): () => void {
    const receive = (event: Event) => listener(supported((event as CustomEvent).detail));
    globalThis.addEventListener('anywhere:language', receive);
    return () => globalThis.removeEventListener('anywhere:language', receive);
  },
};
