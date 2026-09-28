import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

abstract final class OpenChatWindowControls {
  static const MethodChannel _channel = MethodChannel('openchat/window');

  static bool get isSupported =>
      !kIsWeb && defaultTargetPlatform == TargetPlatform.windows;

  static Future<void> minimize() async {
    await _channel.invokeMethod<void>('minimize');
  }

  static Future<void> toggleMaximize() async {
    await _channel.invokeMethod<void>('toggleMaximize');
  }

  static Future<void> close() async {
    await _channel.invokeMethod<void>('close');
  }

  static Future<void> startDragging() async {
    await _channel.invokeMethod<void>('startDragging');
  }
}
