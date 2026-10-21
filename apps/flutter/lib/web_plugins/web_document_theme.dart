import '../ui/design_tokens.g.dart';

/// Host-owned document canvas, independent of any SDK CSS or frontend framework.
/// Common mount roots are canvas, while ordinary tool panels keep their colors.
const webDocumentThemeCss = '''
$webDesignTokensCss
$webControlsCss
html, body { margin: 0; height: 100%; }
html:root, html:root > body, #app, #root, #__next {
  background: var(--aw-web-canvas, var(--aw-fallback-surface)) !important;
  color: var(--aw-text) !important;
}
html:root { color-scheme: normal !important; }
''';

/// Inline important declarations take precedence over framework starter themes,
/// including inline backgrounds. This is a presentation contract, not a sandbox.
String webCanvasScript({required bool transparent}) => '''
(() => {
  const root = document.documentElement;
  if (!root) return;
  root.style.setProperty('--aw-web-canvas', '${transparent ? 'transparent' : 'var(--aw-fallback-surface)'}', 'important');
  root.style.setProperty('color-scheme', 'normal', 'important');
  for (const node of new Set([root, document.body,
      ...document.querySelectorAll('#app, #root, #__next')])) {
    node?.style.setProperty('background', 'var(--aw-web-canvas)', 'important');
    node?.style.setProperty('color', 'var(--aw-text)', 'important');
  }
})();
''';
