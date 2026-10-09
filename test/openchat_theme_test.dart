import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';

double _contrast(Color foreground, Color background) {
  final foregroundLuminance = foreground.computeLuminance();
  final backgroundLuminance = background.computeLuminance();
  final lighter = foregroundLuminance > backgroundLuminance
      ? foregroundLuminance
      : backgroundLuminance;
  final darker = foregroundLuminance > backgroundLuminance
      ? backgroundLuminance
      : foregroundLuminance;
  return (lighter + 0.05) / (darker + 0.05);
}

void main() {
  test('light and dark themes keep semantic contrast and blue primaries', () {
    for (final theme in <ThemeData>[OpenChatTheme.light, OpenChatTheme.dark]) {
      final semantic = theme.extension<OpenChatSemanticColors>()!;
      final palette = theme.extension<OpenChatPalette>()!;

      expect(palette.accent, semantic.primary);
      expect(
        _contrast(semantic.foreground, semantic.background),
        greaterThanOrEqualTo(7),
      );
      expect(
        _contrast(semantic.primaryForeground, semantic.primary),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        _contrast(semantic.focusRing, semantic.surface),
        greaterThanOrEqualTo(3),
      );
      for (final surface in <Color>[
        palette.surface,
        palette.navigation,
        palette.composer,
      ]) {
        expect(_contrast(palette.text, surface), greaterThanOrEqualTo(7));
        expect(
          _contrast(palette.secondaryText, surface),
          greaterThanOrEqualTo(4.5),
        );
        expect(
          _contrast(palette.secondaryIcon, surface),
          greaterThanOrEqualTo(3),
        );
        expect(
          _contrast(palette.disabledIcon, surface),
          greaterThanOrEqualTo(3),
        );
      }
    }
  });

  test('conversation style preserves both semantic theme extensions', () {
    final themed = OpenChatTheme.withConversationStyle(
      OpenChatTheme.dark,
      maxWidth: 760,
      fontFamily: 'Manrope',
    );

    expect(themed.extension<OpenChatSemanticColors>(), isNotNull);
    expect(themed.extension<OpenChatPalette>(), OpenChatPalette.dark);
    expect(themed.extension<OpenChatConversationStyle>()?.maxWidth, 760);
    expect(
      themed.extension<OpenChatConversationStyle>()?.fontFamily,
      'Manrope',
    );
    expect(
      themed.textTheme.bodyLarge?.fontFamily,
      OpenChatTypography.uiFontFamily,
    );
  });
}
