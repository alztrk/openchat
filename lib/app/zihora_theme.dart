import 'package:flutter/material.dart';

class ZihoraPalette extends ThemeExtension<ZihoraPalette> {
  const ZihoraPalette({
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

  static const light = ZihoraPalette(
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

  static const dark = ZihoraPalette(
    navigation: Color(0xFF201E1B),
    surface: Color(0xFF201E1B),
    composer: Color(0xFF201E1B),
    selected: Color(0xFF39332D),
    hover: Color(0xFF39332D),
    border: Color(0xFF3B3732),
    controlBorder: Color(0xFF847B70),
    text: Color(0xFFEEE9E2),
    brandInk: Color(0xFFEEE9E2),
    secondaryText: Color(0xFFB0A79E),
    secondaryIcon: Color(0xFFB0A79E),
    accent: Color(0xFFCF9270),
    accentIcon: Color(0xFFCF9270),
  );

  static ZihoraPalette of(BuildContext context) {
    final palette = Theme.of(context).extension<ZihoraPalette>();
    if (palette == null) {
      throw StateError('ZihoraPalette is missing from the active theme.');
    }
    return palette;
  }

  @override
  ZihoraPalette copyWith({
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
    return ZihoraPalette(
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
  ZihoraPalette lerp(ThemeExtension<ZihoraPalette>? other, double t) {
    if (other is! ZihoraPalette) return this;
    return ZihoraPalette(
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

abstract final class ZihoraSpacing {
  static const pageHorizontal = 32.0;
  static const compactPageHorizontal = 20.0;
  static const sidebarWidth = 320.0;
  static const compactSidebarWidth = 280.0;
  static const expandedRailWidth = 260.0;
  static const compactRailWidth = 72.0;
  static const fullSidebarBreakpoint = 1440.0;
  static const expandedRailBreakpoint = 1600.0;
  static const sidebarBreakpoint = 900.0;
}

abstract final class ZihoraTheme {
  static final ThemeData light = _create(ZihoraPalette.light, Brightness.light);
  static final ThemeData dark = _create(ZihoraPalette.dark, Brightness.dark);

  static ThemeData _create(ZihoraPalette palette, Brightness brightness) {
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
      fontFamily: 'Manrope',
      scaffoldBackgroundColor: palette.surface,
      colorScheme: colorScheme,
      extensions: <ThemeExtension<dynamic>>[palette],
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
      ),
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
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: BorderSide(color: palette.accent),
        ),
      ),
      iconButtonTheme: IconButtonThemeData(
        style: IconButton.styleFrom(
          foregroundColor: palette.secondaryText,
          minimumSize: const Size(36, 36),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          foregroundColor: palette.text,
          side: BorderSide(color: palette.border),
          minimumSize: const Size(36, 36),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
          padding: const EdgeInsets.symmetric(horizontal: 10),
          textStyle: TextStyle(
            color: palette.text,
            fontWeight: FontWeight.w500,
            fontSize: 13,
          ),
        ),
      ),
    );
  }
}
