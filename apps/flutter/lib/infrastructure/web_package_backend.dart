import '../src/rust/api.dart' as api;
import 'web_package_picker.dart';

/// Trusted settings port. Package approval remains in Rust; widgets own only the
/// review presentation and cannot publish a catalog directly.
class WebPackageBackend {
  const WebPackageBackend();
  Future<String?> choose() => chooseWebPackage();
  Future<List<api.WebPackageDto>> list() => api.webPackages();
  Future<api.WebReviewDto> inspect(String path) =>
      api.inspectWebPackage(path: path);
  Future<List<api.WebPackageDto>> approve(String token) =>
      api.approveWebPackage(token: token);
  Future<void> cancelReview() => api.cancelWebReview();
  Future<List<api.WebPackageDto>> change(String id, String operation) =>
      api.changeWebPackage(id: id, operation: operation);
}
