import 'dart:ui' show lerpDouble;

import 'package:flutter/material.dart';

class OpenChatPalette extends ThemeExtension<OpenChatPalette> {
  const OpenChatPalette({
    required this.navigation,
    required this.surface,
    required this.composer,
    required this.selected,
    required this.hover,
    required this.border,
    required this.controlBorder,
    required this.text,
    required this.brandInk,
    required this.secondaryText,
    required this.secondaryIcon,
    required this.accent,
    required this.accentIcon,
  });

  final Color navigation;
  final Color surface;
  final Color composer;
  final Color selected;
  final Color hover;
  final Color border;
  final Color controlBorder;
  final Color text;
  final Color brandInk;
  final Color secondaryText;
  final Color secondaryIcon;
  final Color accent;
  final Color accentIcon;

  Color get disabledForeground =>
      Color.alphaBlend(text.withValues(alpha: 0.48), surface);
  Color get disabledIcon =>
      Color.alphaBlend(secondaryIcon.withValues(alpha: 0.48), surface);
  Color get disabledSurface =>
      Color.alphaBlend(text.withValues(alpha: 0.045), surface);
  Color get disabledBorder =>
      Color.alphaBlend(border.withValues(alpha: 0.58), surface);

  static const light = OpenChatPalette(
    navigation: Color(0xFFE9E3DA),
    surface: Color(0xFFF5F3EE),
    composer: Color(0xFFEEEAE3),
    selected: Color(0xFFDED7CE),
    hover: Color(0xFFF2EDE6),
    border: Color(0xFFDDD7CE),
    controlBorder: Color(0xFF887F74),
    text: Color(0xFF292724),
    brandInk: Color(0xFF262F2B),
    secondaryText: Color(0xFF676057),
    secondaryIcon: Color(0xFF746F68),
    accent: Color(0xFF8E563B),
    accentIcon: Color(0xFF8A4F37),
  );

  static const dark = OpenChatPalette(
    navigation: Color(0xFF1B1917),
    surface: Color(0xFF24211E),
    composer: Color(0xFF2B2723),
    selected: Color(0xFF39332D),
    hover: Color(0xFF322D28),
    border: Color(0xFF403A34),
    controlBorder: Color(0xFF847B70),
    text: Color(0xFFEEE9E2),
    brandInk: Color(0xFFEEE9E2),
    secondaryText: Color(0xFFB0A79E),
    secondaryIcon: Color(0xFFB0A79E),
    accent: Color(0xFFCF9270),
    accentIcon: Color(0xFFCF9270),
  );

  static OpenChatPalette of(BuildContext context) {
    final palette = Theme.of(context).extension<OpenChatPalette>();
    if (palette == null) {
      throw StateError('OpenChatPalette is missing from the active theme.');
    }
    return palette;
  }

  @override
  OpenChatPalette copyWith({
    Color? navigation,
    Color? surface,
    Color? composer,
    Color? selected,
    Color? hover,
    Color? border,
    Color? controlBorder,
    Color? text,
    Color? brandInk,
    Color? secondaryText,
    Color? secondaryIcon,
    Color? accent,
    Color? accentIcon,
  }) {
    return OpenChatPalette(
      navigation: navigation ?? this.navigation,
      surface: surface ?? this.surface,
      composer: composer ?? this.composer,
      selected: selected ?? this.selected,
      hover: hover ?? this.hover,
      border: border ?? this.border,
      controlBorder: controlBorder ?? this.controlBorder,
      text: text ?? this.text,
      brandInk: brandInk ?? this.brandInk,
      secondaryText: secondaryText ?? this.secondaryText,
      secondaryIcon: secondaryIcon ?? this.secondaryIcon,
      accent: accent ?? this.accent,
      accentIcon: accentIcon ?? this.accentIcon,
    );
  }

  @override
  OpenChatPalette lerp(ThemeExtension<OpenChatPalette>? other, double t) {
    if (other is! OpenChatPalette) return this;
    return OpenChatPalette(
      navigation: Color.lerp(navigation, other.navigation, t) ?? navigation,
      surface: Color.lerp(surface, other.surface, t) ?? surface,
      composer: Color.lerp(composer, other.composer, t) ?? composer,
      selected: Color.lerp(selected, other.selected, t) ?? selected,
      hover: Color.lerp(hover, other.hover, t) ?? hover,
      border: Color.lerp(border, other.border, t) ?? border,
      controlBorder:
          Color.lerp(controlBorder, other.controlBorder, t) ?? controlBorder,
      text: Color.lerp(text, other.text, t) ?? text,
      brandInk: Color.lerp(brandInk, other.brandInk, t) ?? brandInk,
      secondaryText:
          Color.lerp(secondaryText, other.secondaryText, t) ?? secondaryText,
      secondaryIcon:
          Color.lerp(secondaryIcon, other.secondaryIcon, t) ?? secondaryIcon,
      accent: Color.lerp(accent, other.accent, t) ?? accent,
      accentIcon: Color.lerp(accentIcon, other.accentIcon, t) ?? accentIcon,
    );
  }
}

class OpenChatConversationStyle
    extends ThemeExtension<OpenChatConversationStyle> {
  const OpenChatConversationStyle({
    this.maxWidth = OpenChatSpacing.conversationMaxWidth,
    this.fontFamily = 'Manrope',
  });

  final double maxWidth;
  final String fontFamily;

  static OpenChatConversationStyle of(BuildContext context) {
    final style = Theme.of(context).extension<OpenChatConversationStyle>();
    if (style == null) {
      throw StateError(
        'OpenChatConversationStyle is missing from the active theme.',
      );
    }
    return style;
  }

  @override
  OpenChatConversationStyle copyWith({double? maxWidth, String? fontFamily}) {
    return OpenChatConversationStyle(
      maxWidth: maxWidth ?? this.maxWidth,
      fontFamily: fontFamily ?? this.fontFamily,
    );
  }

  @override
  OpenChatConversationStyle lerp(
    ThemeExtension<OpenChatConversationStyle>? other,
    double t,
  ) {
    if (other is! OpenChatConversationStyle) return this;
    return OpenChatConversationStyle(
      maxWidth: lerpDouble(maxWidth, other.maxWidth, t) ?? maxWidth,
      fontFamily: t < 0.5 ? fontFamily : other.fontFamily,
    );
  }
}

abstract final class OpenChatSpacing {
  static const pageHorizontal = 32.0;
  static const compactPageHorizontal = 20.0;
  static const sidebarWidth = 320.0;
  static const compactSidebarWidth = 280.0;
  static const expandedRailWidth = 260.0;
  static const compactRailWidth = 72.0;
  static const collapsedSidebarWidth = 240.0;
  static const conversationMaxWidth = 920.0;
  static const fullSidebarBreakpoint = 1440.0;
  static const expandedRailBreakpoint = 1600.0;
  static const sidebarBreakpoint = 900.0;
}

abstract final class OpenChatTheme {
  static final ThemeData light = _create(
    OpenChatPalette.light,
    Brightness.light,
  );
  static final ThemeData dark = _create(OpenChatPalette.dark, Brightness.dark);

  static ThemeData _create(
    OpenChatPalette palette,
    Brightness brightness, {
    String fontFamily = 'Manrope',
  }) {
    final colorScheme = ColorScheme.fromSeed(
      seedColor: palette.accent,
      brightness: brightness,
      surface: palette.surface,
      primary: palette.accent,
      onPrimary: palette.surface,
      onSurface: palette.text,
      outline: palette.border,
    );

    return ThemeData(
      useMaterial3: true,
      brightness: brightness,
      fontFamily: fontFamily,
      scaffoldBackgroundColor: palette.surface,
      colorScheme: colorScheme,
      extensions: <ThemeExtension<dynamic>>[
        palette,
        const OpenChatConversationStyle(),
      ],
      textTheme: TextTheme(
        headlineSmall: TextStyle(
          color: palette.text,
          fontSize: 24,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 1.3,
        ),
        titleLarge: TextStyle(
          color: palette.text,
          fontSize: 18,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 1.2,
        ),
        titleMedium: TextStyle(
          color: palette.text,
          fontSize: 16,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 1.4,
        ),
        bodyLarge: TextStyle(
          color: palette.text,
          fontSize: 15,
          fontWeight: FontWeight.w500,
          letterSpacing: 0,
          height: 1.45,
        ),
        bodyMedium: TextStyle(
          color: palette.secondaryText,
          fontSize: 13,
          fontWeight: FontWeight.w500,
          letterSpacing: 0,
          height: 1.5,
        ),
        bodySmall: TextStyle(
          color: palette.secondaryText,
          fontSize: 12,
          fontWeight: FontWeight.w500,
          letterSpacing: 0,
          height: 1.45,
        ),
        labelLarge: TextStyle(
          color: palette.text,
          fontSize: 13,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 1.4,
        ),
      ).apply(fontFamily: fontFamily),
      dividerColor: palette.border,
      inputDecorationTheme: InputDecorationTheme(
        hintStyle: TextStyle(color: palette.secondaryText, fontSize: 13),
        filled: true,
        fillColor: palette.surface,
        contentPadding: const EdgeInsets.symmetric(horizontal: 12),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: BorderSide(color: palette.controlBorder),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: BorderSide(color: palette.controlBorder),
        ),
        disabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: BorderSide(color: palette.disabledBorder),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: BorderSide(color: palette.accent),
        ),
      ),
      iconButtonTheme: IconButtonThemeData(
        style: IconButton.styleFrom(
          foregroundColor: palette.secondaryText,
          disabledForegroundColor: palette.disabledIcon,
          minimumSize: const Size(36, 36),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          disabledForegroundColor: palette.disabledForeground,
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style:
            OutlinedButton.styleFrom(
              foregroundColor: palette.text,
              disabledForegroundColor: palette.disabledForeground,
              disabledBackgroundColor: palette.disabledSurface,
              side: BorderSide(color: palette.border),
              minimumSize: const Size(36, 36),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(8),
              ),
              padding: const EdgeInsets.symmetric(horizontal: 10),
              textStyle: TextStyle(
                color: palette.text,
                fontWeight: FontWeight.w500,
                fontSize: 13,
              ),
            ).copyWith(
              side: WidgetStateProperty.resolveWith<BorderSide?>((states) {
                final color = states.contains(WidgetState.disabled)
                    ? palette.disabledBorder
                    : palette.border;
                return BorderSide(color: color);
              }),
            ),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          disabledForegroundColor: palette.disabledForeground,
          disabledBackgroundColor: palette.disabledSurface,
        ),
      ),
    );
  }

  static ThemeData withConversationStyle(
    ThemeData theme, {
    required double maxWidth,
    required String fontFamily,
  }) {
    final palette = theme.extension<OpenChatPalette>();
    if (palette == null) {
      throw StateError('OpenChatPalette is missing from the active theme.');
    }
    return _create(palette, theme.brightness, fontFamily: fontFamily).copyWith(
      extensions: <ThemeExtension<dynamic>>[
        palette,
        OpenChatConversationStyle(maxWidth: maxWidth, fontFamily: fontFamily),
      ],
    );
  }
}
