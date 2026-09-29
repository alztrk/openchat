import 'package:flutter/painting.dart';

final class OpenChatTextScaler extends TextScaler {
  const OpenChatTextScaler(this.platformScaler, this.preferenceScale);

  final TextScaler platformScaler;
  final double preferenceScale;

  @override
  double scale(double fontSize) =>
      platformScaler.scale(fontSize) * preferenceScale;

  @override
  double get textScaleFactor => platformScaler.scale(1) * preferenceScale;

  @override
  bool operator ==(Object other) =>
      other is OpenChatTextScaler &&
      other.platformScaler == platformScaler &&
      other.preferenceScale == preferenceScale;

  @override
  int get hashCode => Object.hash(platformScaler, preferenceScale);
}
