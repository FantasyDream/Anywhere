import 'package:anywhere/tools/tool_page.dart';
import 'package:anywhere/tools/tool_page_stack.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

class _Page extends ToolPageSession {
  int disposals = 0;
  final visibility = <bool>[];
  @override
  String get id => 'test.page';
  @override
  String get title => 'Test';
  @override
  void setVisible(bool visible) => visibility.add(visible);
  @override
  void dispose() {
    disposals++;
    super.dispose();
  }

  @override
  Widget build(BuildContext context,
          {required VoidCallback onBack,
          required int presentation,
          required bool visible,
          String? notice,
          Future<void> Function()? onCopy}) =>
      const SizedBox();
}

void main() {
  test(
      'return preserves the parent and replacement disposes all retained pages',
      () {
    final stack = ToolPageStack();
    final root = _Page(), child = _Page(), replacement = _Page();
    stack.replace(root);
    stack.push(child);
    expect(root.visibility, [false]);
    expect(root.disposals, 0);
    stack.back();
    expect(stack.current, same(root));
    expect(root.visibility, [false, true]);
    expect(child.disposals, 1);
    final next = _Page();
    stack.push(next);
    stack.replace(replacement);
    expect(root.disposals, 1);
    expect(next.disposals, 1);
    stack.replace(replacement);
    expect(replacement.disposals, 0);
    stack.replace(null);
    expect(replacement.disposals, 1);
    expect(stack.current, isNull);
  });

  test('nested page limit rejects and disposes only the offered page', () {
    final stack = ToolPageStack();
    final pages = List.generate(5, (_) => _Page());
    for (final page in pages) {
      stack.push(page);
    }
    final excess = _Page();
    expect(() => stack.push(excess), throwsStateError);
    expect(excess.disposals, 1);
    expect(stack.current, same(pages.last));
    expect(pages.every((page) => page.disposals == 0), true);
    stack.replace(null);
    expect(pages.every((page) => page.disposals == 1), true);
  });
}
