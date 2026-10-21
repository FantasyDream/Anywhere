import 'package:flutter/services.dart';

/// OS file selection is user-initiated and never exported through the Web SDK.
Future<String?> chooseWebPackage() => const MethodChannel('anywhere/window')
    .invokeMethod<String>('chooseWebPackage');
