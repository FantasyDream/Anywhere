import '../infrastructure/desktop_appearance.dart';
import '../ui/design_tokens.g.dart';

/// Host-owned document canvas, independent of any SDK CSS or frontend framework.
/// Common mount roots are canvas, while ordinary tool panels keep their colors.
const webDocumentThemeCss = '''
$webDesignTokensCss
$webControlsCss
html:root, html:root > body, #app, #root, #__next {
  background: var(--aw-web-canvas, var(--aw-fallback-surface)) !important;
  color: var(--aw-text) !important;
}
''';

/// Inline important declarations take precedence over framework starter themes,
/// including inline backgrounds. This is a presentation contract, not a sandbox.
String webCanvasScript({
  required bool transparent,
  DesktopAppearanceSnapshot? appearance,
}) {
  final current = appearance ?? DesktopAppearance.current.snapshot;
  final brightness =
      current.brightness == AppearanceBrightness.light ? 'light' : 'dark';
  final contrast = current.increasedContrast ? 'increased' : 'standard';
  return '''
(() => {
  const root = document.documentElement;
  if (!root) return;
  root.dataset.awAppearance = '$brightness';
  root.dataset.awContrast = '$contrast';
  root.dataset.awReduceMotion = '${current.reduceMotion}';
  root.style.setProperty('--aw-web-canvas', '${transparent ? 'transparent' : 'var(--aw-fallback-surface)'}', 'important');
  root.style.setProperty('color-scheme', '$brightness', 'important');
  for (const node of new Set([root, document.body,
      ...document.querySelectorAll('#app, #root, #__next')])) {
    node?.style.setProperty('background', 'var(--aw-web-canvas)', 'important');
    node?.style.setProperty('color', 'var(--aw-text)', 'important');
  }
})();
''';
}
