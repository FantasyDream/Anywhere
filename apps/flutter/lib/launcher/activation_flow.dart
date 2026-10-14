import 'dart:async';
import '../infrastructure/runtime_policy.dart';
import '../src/rust/api.dart';
import 'activation_prefill.dart';

/// Host presentation of activation input. Eligibility and the 15s clipboard
/// policy remain in Rust; this flow owns the grace timer and interaction veto.
class ActivationFlow {
  InputModeDto mode = InputModeDto.environment;
  ContextSourceDto? source;
  bool pendingAppHome = false;
  Future<void>? pendingPrefill;
  bool reconsiderAfterCheck = false;
  int _epoch = 0;
  bool get prefillAllowed => _allowed;
  bool _allowed = false, _gracePassed = false;
  Timer? _grace;

  void begin(
      {required bool prefill,
      required bool Function() current,
      required void Function() reconsider}) {
    stopPrefill();
    mode = InputModeDto.environment;
    source = null;
    pendingAppHome = false;
    pendingPrefill = null;
    reconsiderAfterCheck = false;
    _allowed = prefill;
    _gracePassed = false;
    if (prefill) {
      _grace = Timer(
          const Duration(milliseconds: RuntimePolicy.selectionPrefillGraceMs),
          () {
        if (!current()) return;
        _gracePassed = true;
        reconsider();
      });
    }
  }

  Future<ActivationPrefill?> take(List<ContextCaptureDto> captures, int now,
      {required ChooseActivationInput? choose,
      required bool Function() interacted,
      required bool Function() visible,
      required int Function() nowMillis,
      required bool selectedEnabled,
      required bool clipboardEnabled}) async {
    if (!_allowed || interacted() || !visible() || choose == null) return null;
    final epoch = _epoch;
    final offer = await ActivationPrefill.choose(captures, now,
        choose: choose,
        selectedEnabled: selectedEnabled,
        clipboardEnabled: clipboardEnabled,
        allowPendingSelection: _gracePassed);
    if (epoch != _epoch || !_allowed || interacted() || !visible()) return null;
    if (offer?.validUntilMs != null && nowMillis() > offer!.validUntilMs!) {
      return null;
    }
    if (offer != null) {
      stopPrefill();
      mode = InputModeDto.content;
      source = offer.source;
    }
    return offer;
  }

  void stopPrefill() {
    _epoch++;
    _allowed = false;
    _grace?.cancel();
    _grace = null;
  }

  void dispose() => stopPrefill();
}
