import 'package:anywhere/web_plugins/web_view_appearance.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:flutter_test/flutter_test.dart';

class TestWebController extends PlatformInAppWebViewController {
  TestWebController(int id)
      : super.implementation(
            PlatformInAppWebViewControllerCreationParams(id: id));
}

void main() {
  test('load callbacks share a platform view despite distinct SDK facades', () {
    final platform = TestWebController(1);
    final created = InAppWebViewController.fromPlatform(platform: platform);
    final loaded = InAppWebViewController.fromPlatform(platform: platform);
    expect(identical(created, loaded), isFalse);
    expect(sharesWebView(created, loaded), isTrue);
    // A replacement may reuse an id, but must never qualify as the old view.
    final replacement =
        InAppWebViewController.fromPlatform(platform: TestWebController(1));
    expect(sharesWebView(created, replacement), isFalse);
    expect(sharesWebView(null, loaded), isFalse);
  });
}
