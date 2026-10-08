import 'dart:ui' show lerpDouble;

import 'package:flutter/material.dart';

class OpenChatPalette extends ThemeExtension<OpenChatPalette> {
  const OpenChatPalette({
    required this.background,
    required this.navigation,
    required this.surface,
    required this.raisedSurface,
    required this.composer,
    required this.selected,
    required this.hover,
    required this.border,
    required this.subtleBorder,
    required this.controlBorder,
    required this.text,
    required this.brandInk,
    required this.secondaryText,
    required this.secondaryIcon,
    required this.accent,
    required this.accentHover,
    required this.accentActive,
    required this.accentForeground,
    required this.accentIcon,
    required this.destructive,
    required this.warning,
    required this.success,
    required this.info,
    required this.focusRing,
  });

  final Color background;
  final Color navigation;
  final Color surface;
  final Color raisedSurface;
  final Color composer;
  final Color selected;
  final Color hover;
  final Color border;
  final Color subtleBorder;
  final Color controlBorder;
  final Color text;
  final Color brandInk;
  final Color secondaryText;
  final Color secondaryIcon;
  final Color accent;
  final Color accentHover;
  final Color accentActive;
  final Color accentForeground;
  final Color accentIcon;
  final Color destructive;
  final Color warning;
  final Color success;
  final Color info;
  final Color focusRing;

  Color get disabledForeground =>
      Color.alphaBlend(text.withValues(alpha: 0.56), surface);
  Color get disabledIcon => disabledForeground;
  Color get disabledSurface =>
      Color.alphaBlend(text.withValues(alpha: 0.045), surface);
  Color get disabledBorder =>
      Color.alphaBlend(border.withValues(alpha: 0.58), surface);

  static const light = OpenChatPalette(
    background: Color(0xFFF6F3EF),
    navigation: Color(0xFFEEEAE4),
    surface: Color(0xFFFFFCF8),
    raisedSurface: Color(0xFFFFFFFF),
    composer: Color(0xFFF1EBE4),
    selected: Color(0xFFE7DED3),
    hover: Color(0xFFEFE7DE),
    border: Color(0xFFCDC3B9),
    subtleBorder: Color(0xFFE3DBD2),
    controlBorder: Color(0xFF81766B),
    text: Color(0xFF24211E),
    brandInk: Color(0xFF24211E),
    secondaryText: Color(0xFF625B54),
    secondaryIcon: Color(0xFF746B62),
    accent: Color(0xFF8E563B),
    accentHover: Color(0xFF78452F),
    accentActive: Color(0xFF603B2B),
    accentForeground: Color(0xFFFFFFFF),
    accentIcon: Color(0xFF8E563B),
    destructive: Color(0xFFA53030),
    warning: Color(0xFF7A5200),
    success: Color(0xFF176B45),
    info: Color(0xFF0B5D81),
    focusRing: Color(0xFF005A74),
  );

  static const dark = OpenChatPalette(
    background: Color(0xFF181614),
    navigation: Color(0xFF211F1C),
    surface: Color(0xFF211F1C),
    raisedSurface: Color(0xFF2A2723),
    composer: Color(0xFF2D2925),
    selected: Color(0xFF3A332C),
    hover: Color(0xFF332E29),
    border: Color(0xFF4A443E),
    subtleBorder: Color(0xFF39342F),
    controlBorder: Color(0xFF8E8175),
    text: Color(0xFFF4EEE7),
    brandInk: Color(0xFFF4EEE7),
    secondaryText: Color(0xFFB7ADA2),
    secondaryIcon: Color(0xFFC3B8AC),
    accent: Color(0xFFD08B68),
    accentHover: Color(0xFFE39A75),
    accentActive: Color(0xFFF0AD89),
    accentForeground: Color(0xFF2C1B14),
    accentIcon: Color(0xFFD08B68),
    destructive: Color(0xFFF08C84),
    warning: Color(0xFFF4C95D),
    success: Color(0xFF70D39A),
    info: Color(0xFF80C8E8),
    focusRing: Color(0xFF8DDCFF),
  );

  static const highContrastLight = OpenChatPalette(
    background: Color(0xFFFFFFFF),
    navigation: Color(0xFFFFFFFF),
    surface: Color(0xFFFFFFFF),
    raisedSurface: Color(0xFFFFFFFF),
    composer: Color(0xFFF4F4F4),
    selected: Color(0xFFD9D9D9),
    hover: Color(0xFFE8E8E8),
    border: Color(0xFF595959),
    subtleBorder: Color(0xFF767676),
    controlBorder: Color(0xFF404040),
    text: Color(0xFF000000),
    brandInk: Color(0xFF000000),
    secondaryText: Color(0xFF404040),
    secondaryIcon: Color(0xFF404040),
    accent: Color(0xFF0033B8),
    accentHover: Color(0xFF00258A),
    accentActive: Color(0xFF001B66),
    accentForeground: Color(0xFFFFFFFF),
    accentIcon: Color(0xFF0033B8),
    destructive: Color(0xFF9B0000),
    warning: Color(0xFF754300),
    success: Color(0xFF005A2B),
    info: Color(0xFF00527A),
    focusRing: Color(0xFF0033B8),
  );

  static const highContrastDark = OpenChatPalette(
    background: Color(0xFF000000),
    navigation: Color(0xFF000000),
    surface: Color(0xFF000000),
    raisedSurface: Color(0xFF111111),
    composer: Color(0xFF111111),
    selected: Color(0xFF333333),
    hover: Color(0xFF252525),
    border: Color(0xFFB8B8B8),
    subtleBorder: Color(0xFF999999),
    controlBorder: Color(0xFFD9D9D9),
    text: Color(0xFFFFFFFF),
    brandInk: Color(0xFFFFFFFF),
    secondaryText: Color(0xFFE6E6E6),
    secondaryIcon: Color(0xFFE6E6E6),
    accent: Color(0xFF91BEFF),
    accentHover: Color(0xFFB0D0FF),
    accentActive: Color(0xFFD0E2FF),
    accentForeground: Color(0xFF000000),
    accentIcon: Color(0xFF91BEFF),
    destructive: Color(0xFFFF9999),
    warning: Color(0xFFFFD080),
    success: Color(0xFF8AE6B2),
    info: Color(0xFF91D9FF),
    focusRing: Color(0xFFFFFF00),
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
    Color? background,
    Color? navigation,
    Color? surface,
    Color? raisedSurface,
    Color? composer,
    Color? selected,
    Color? hover,
    Color? border,
    Color? subtleBorder,
    Color? controlBorder,
    Color? text,
    Color? brandInk,
    Color? secondaryText,
    Color? secondaryIcon,
    Color? accent,
    Color? accentHover,
    Color? accentActive,
    Color? accentForeground,
    Color? accentIcon,
    Color? destructive,
    Color? warning,
    Color? success,
    Color? info,
    Color? focusRing,
  }) {
    return OpenChatPalette(
      background: background ?? this.background,
      navigation: navigation ?? this.navigation,
      surface: surface ?? this.surface,
      raisedSurface: raisedSurface ?? this.raisedSurface,
      composer: composer ?? this.composer,
      selected: selected ?? this.selected,
      hover: hover ?? this.hover,
      border: border ?? this.border,
      subtleBorder: subtleBorder ?? this.subtleBorder,
      controlBorder: controlBorder ?? this.controlBorder,
      text: text ?? this.text,
      brandInk: brandInk ?? this.brandInk,
      secondaryText: secondaryText ?? this.secondaryText,
      secondaryIcon: secondaryIcon ?? this.secondaryIcon,
      accent: accent ?? this.accent,
      accentHover: accentHover ?? this.accentHover,
      accentActive: accentActive ?? this.accentActive,
      accentForeground: accentForeground ?? this.accentForeground,
      accentIcon: accentIcon ?? this.accentIcon,
      destructive: destructive ?? this.destructive,
      warning: warning ?? this.warning,
      success: success ?? this.success,
      info: info ?? this.info,
      focusRing: focusRing ?? this.focusRing,
    );
  }

  @override
  OpenChatPalette lerp(ThemeExtension<OpenChatPalette>? other, double t) {
    if (other is! OpenChatPalette) return this;
    return OpenChatPalette(
      background: Color.lerp(background, other.background, t) ?? background,
      navigation: Color.lerp(navigation, other.navigation, t) ?? navigation,
      surface: Color.lerp(surface, other.surface, t) ?? surface,
      raisedSurface:
          Color.lerp(raisedSurface, other.raisedSurface, t) ?? raisedSurface,
      composer: Color.lerp(composer, other.composer, t) ?? composer,
      selected: Color.lerp(selected, other.selected, t) ?? selected,
      hover: Color.lerp(hover, other.hover, t) ?? hover,
      border: Color.lerp(border, other.border, t) ?? border,
      subtleBorder:
          Color.lerp(subtleBorder, other.subtleBorder, t) ?? subtleBorder,
      controlBorder:
          Color.lerp(controlBorder, other.controlBorder, t) ?? controlBorder,
      text: Color.lerp(text, other.text, t) ?? text,
      brandInk: Color.lerp(brandInk, other.brandInk, t) ?? brandInk,
      secondaryText:
          Color.lerp(secondaryText, other.secondaryText, t) ?? secondaryText,
      secondaryIcon:
          Color.lerp(secondaryIcon, other.secondaryIcon, t) ?? secondaryIcon,
      accent: Color.lerp(accent, other.accent, t) ?? accent,
      accentHover: Color.lerp(accentHover, other.accentHover, t) ?? accentHover,
      accentActive:
          Color.lerp(accentActive, other.accentActive, t) ?? accentActive,
      accentForeground:
          Color.lerp(accentForeground, other.accentForeground, t) ??
          accentForeground,
      accentIcon: Color.lerp(accentIcon, other.accentIcon, t) ?? accentIcon,
      destructive: Color.lerp(destructive, other.destructive, t) ?? destructive,
      warning: Color.lerp(warning, other.warning, t) ?? warning,
      success: Color.lerp(success, other.success, t) ?? success,
      info: Color.lerp(info, other.info, t) ?? info,
      focusRing: Color.lerp(focusRing, other.focusRing, t) ?? focusRing,
    );
  }
}

/// Compatibility view of semantic roles derived from the canonical palette.
///
/// New components should use [OpenChatPalette] directly.
class OpenChatSemanticColors extends ThemeExtension<OpenChatSemanticColors> {
  const OpenChatSemanticColors._(this._palette);

  final OpenChatPalette _palette;

  static final light = OpenChatSemanticColors._(OpenChatPalette.light);
  static final dark = OpenChatSemanticColors._(OpenChatPalette.dark);

  @override
  OpenChatSemanticColors copyWith() => this;

  @override
  OpenChatSemanticColors lerp(
    ThemeExtension<OpenChatSemanticColors>? other,
    double t,
  ) => other is OpenChatSemanticColors
      ? OpenChatSemanticColors._(_palette.lerp(other._palette, t))
      : this;

  Color get background => _palette.background;
  Color get foreground => _palette.text;
  Color get muted => _palette.selected;
  Color get mutedForeground => _palette.secondaryText;
  Color get surface => _palette.surface;
  Color get elevatedSurface => _palette.raisedSurface;
  Color get border => _palette.border;
  Color get subtleBorder => _palette.subtleBorder;
  Color get primary => _palette.accent;
  Color get primaryHover => _palette.accentHover;
  Color get primaryActive => _palette.accentActive;
  Color get primaryForeground => _palette.accentForeground;
  Color get secondary => _palette.selected;
  Color get accent => _palette.accent;
  Color get destructive => _palette.destructive;
  Color get warning => _palette.warning;
  Color get success => _palette.success;
  Color get info => _palette.info;
  Color get focusRing => _palette.focusRing;

  static OpenChatSemanticColors of(BuildContext context) {
    final colors = Theme.of(context).extension<OpenChatSemanticColors>();
    if (colors == null) {
      throw StateError(
        'OpenChatSemanticColors is missing from the active theme.',
      );
    }
    return colors;
  }
}

class OpenChatConversationStyle
    extends ThemeExtension<OpenChatConversationStyle> {
  const OpenChatConversationStyle({
    this.maxWidth = OpenChatSpacing.conversationMaxWidth,
    this.fontFamily = OpenChatTypography.uiFontFamily,
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
  static const xxs = 4.0;
  static const xs = 8.0;
  static const sm = 12.0;
  static const md = 16.0;
  static const lg = 24.0;
  static const xl = 32.0;
  static const xxl = 40.0;
  static const section = 48.0;
  static const pageHorizontal = 32.0;
  static const compactPageHorizontal = 16.0;
  static const appTitleBarHeight = 40.0;
  static const conversationHeaderHeight = 40.0;
  static const sidebarWidth = 320.0;
  static const compactSidebarWidth = 300.0;
  static const expandedRailWidth = 260.0;
  static const compactRailWidth = 52.0;
  static const collapsedSidebarWidth = 260.0;
  static const conversationMaxWidth = 920.0;
  static const composerMaxWidth = 720.0;
  static const mainSurfaceInset = 8.0;
  static const composerBottomInset = 16.0;
  static const fullSidebarBreakpoint = 1440.0;
  static const expandedRailBreakpoint = 1600.0;
  static const sidebarBreakpoint = 900.0;
}

abstract final class OpenChatRadii {
  static const control = 8.0;
  static const menu = 8.0;
  static const card = 12.0;
  static const panel = 12.0;
  static const dialog = 16.0;
}

abstract final class OpenChatTypography {
  static const uiFontFamily = 'Source Sans 3';
  static const codeFontFamily = 'Source Code Pro';
  static const conversation = 16.0;
  static const pageTitle = 24.0;
  static const sectionTitle = 20.0;
  static const componentTitle = 16.0;
  static const body = 14.0;
  static const metadata = 12.0;
  static const code = 13.0;
}

abstract final class OpenChatTheme {
  static final ThemeData light = _create(
    OpenChatPalette.light,
    Brightness.light,
  );
  static final ThemeData dark = _create(OpenChatPalette.dark, Brightness.dark);
  static final ThemeData highContrastLight = _create(
    OpenChatPalette.highContrastLight,
    Brightness.light,
  );
  static final ThemeData highContrastDark = _create(
    OpenChatPalette.highContrastDark,
    Brightness.dark,
  );

  static ThemeData _create(
    OpenChatPalette palette,
    Brightness brightness, {
    String fontFamily = OpenChatTypography.uiFontFamily,
  }) {
    final semantic = OpenChatSemanticColors._(palette);
    final colorScheme = ColorScheme.fromSeed(
      seedColor: semantic.primary,
      brightness: brightness,
      surface: semantic.surface,
      primary: semantic.primary,
      onPrimary: semantic.primaryForeground,
      onSurface: semantic.foreground,
      outline: semantic.border,
      error: semantic.destructive,
    );

    return ThemeData(
      useMaterial3: true,
      brightness: brightness,
      fontFamily: fontFamily,
      scaffoldBackgroundColor: semantic.background,
      colorScheme: colorScheme,
      extensions: <ThemeExtension<dynamic>>[
        palette,
        semantic,
        const OpenChatConversationStyle(),
      ],
      textTheme: TextTheme(
        headlineSmall: TextStyle(
          color: semantic.foreground,
          fontSize: OpenChatTypography.pageTitle,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 32 / OpenChatTypography.pageTitle,
        ),
        titleLarge: TextStyle(
          color: semantic.foreground,
          fontSize: OpenChatTypography.sectionTitle,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 28 / OpenChatTypography.sectionTitle,
        ),
        titleMedium: TextStyle(
          color: semantic.foreground,
          fontSize: OpenChatTypography.componentTitle,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 24 / OpenChatTypography.componentTitle,
        ),
        bodyLarge: TextStyle(
          color: semantic.foreground,
          fontSize: OpenChatTypography.conversation,
          fontWeight: FontWeight.w400,
          letterSpacing: 0,
          height: 26 / OpenChatTypography.conversation,
        ),
        bodyMedium: TextStyle(
          color: semantic.mutedForeground,
          fontSize: OpenChatTypography.body,
          fontWeight: FontWeight.w500,
          letterSpacing: 0,
          height: 20 / OpenChatTypography.body,
        ),
        bodySmall: TextStyle(
          color: semantic.mutedForeground,
          fontSize: OpenChatTypography.metadata,
          fontWeight: FontWeight.w500,
          letterSpacing: 0,
          height: 16 / OpenChatTypography.metadata,
        ),
        labelLarge: TextStyle(
          color: semantic.foreground,
          fontSize: OpenChatTypography.body,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 20 / OpenChatTypography.body,
        ),
      ).apply(fontFamily: fontFamily),
      dividerColor: semantic.border,
      inputDecorationTheme: InputDecorationTheme(
        hintStyle: TextStyle(
          color: semantic.mutedForeground,
          fontSize: OpenChatTypography.body,
        ),
        labelStyle: TextStyle(
          color: semantic.mutedForeground,
          fontSize: OpenChatTypography.body,
        ),
        floatingLabelStyle: TextStyle(
          color: semantic.primary,
          fontSize: OpenChatTypography.body,
        ),
        filled: true,
        fillColor: semantic.surface,
        contentPadding: const EdgeInsets.symmetric(horizontal: 12),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.control),
          borderSide: BorderSide(color: semantic.border),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.control),
          borderSide: BorderSide(color: semantic.border),
        ),
        disabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.control),
          borderSide: BorderSide(color: palette.disabledBorder),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.control),
          borderSide: BorderSide(color: semantic.focusRing, width: 1.4),
        ),
        errorBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.control),
          borderSide: BorderSide(color: colorScheme.error),
        ),
        focusedErrorBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.control),
          borderSide: BorderSide(color: colorScheme.error, width: 1.4),
        ),
      ),
      iconButtonTheme: IconButtonThemeData(
        style: ButtonStyle(
          foregroundColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.disabled)
                ? palette.disabledIcon
                : palette.secondaryText,
          ),
          minimumSize: const WidgetStatePropertyAll(Size(40, 40)),
          shape: WidgetStatePropertyAll(
            RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(OpenChatRadii.control),
            ),
          ),
          overlayColor: _controlOverlay(palette),
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: ButtonStyle(
          foregroundColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.disabled)
                ? palette.disabledForeground
                : palette.text,
          ),
          minimumSize: const WidgetStatePropertyAll(Size(40, 40)),
          padding: const WidgetStatePropertyAll(
            EdgeInsets.symmetric(horizontal: 10),
          ),
          shape: WidgetStatePropertyAll(
            RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(OpenChatRadii.control),
            ),
          ),
          overlayColor: _controlOverlay(palette),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: ButtonStyle(
          foregroundColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.disabled)
                ? palette.disabledForeground
                : palette.text,
          ),
          backgroundColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.disabled)
                ? palette.disabledSurface
                : Colors.transparent,
          ),
          side: WidgetStateProperty.resolveWith((states) {
            final color = states.contains(WidgetState.disabled)
                ? palette.disabledBorder
                : states.contains(WidgetState.focused)
                ? semantic.focusRing
                : semantic.border;
            return BorderSide(color: color);
          }),
          minimumSize: const WidgetStatePropertyAll(Size(40, 40)),
          padding: const WidgetStatePropertyAll(
            EdgeInsets.symmetric(horizontal: 10),
          ),
          shape: WidgetStatePropertyAll(
            RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(OpenChatRadii.control),
            ),
          ),
          textStyle: WidgetStatePropertyAll(
            TextStyle(
              color: semantic.foreground,
              fontFamily: fontFamily,
              fontWeight: FontWeight.w500,
              fontSize: OpenChatTypography.body,
            ),
          ),
          overlayColor: _controlOverlay(palette),
        ),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: ButtonStyle(
          foregroundColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.disabled)
                ? palette.disabledForeground
                : colorScheme.onPrimary,
          ),
          backgroundColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.disabled)
                ? palette.disabledSurface
                : colorScheme.primary,
          ),
          minimumSize: const WidgetStatePropertyAll(Size(40, 40)),
          padding: const WidgetStatePropertyAll(
            EdgeInsets.symmetric(horizontal: 12),
          ),
          shape: WidgetStatePropertyAll(
            RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(OpenChatRadii.control),
            ),
          ),
          textStyle: WidgetStatePropertyAll(
            TextStyle(
              fontWeight: FontWeight.w600,
              fontSize: OpenChatTypography.body,
              fontFamily: fontFamily,
            ),
          ),
          overlayColor: _primaryOverlay(palette),
        ),
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ButtonStyle(
          foregroundColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.disabled)
                ? palette.disabledForeground
                : palette.text,
          ),
          backgroundColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.disabled)
                ? palette.disabledSurface
                : palette.selected,
          ),
          elevation: const WidgetStatePropertyAll(0),
          minimumSize: const WidgetStatePropertyAll(Size(40, 40)),
          padding: const WidgetStatePropertyAll(
            EdgeInsets.symmetric(horizontal: 12),
          ),
          shape: WidgetStatePropertyAll(
            RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(OpenChatRadii.control),
            ),
          ),
          overlayColor: _controlOverlay(palette),
        ),
      ),
      dialogTheme: DialogThemeData(
        backgroundColor: semantic.elevatedSurface,
        surfaceTintColor: Colors.transparent,
        elevation: 8,
        shadowColor: Colors.black.withValues(alpha: 0.16),
        insetPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 24),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.dialog),
        ),
        titleTextStyle: TextStyle(
          color: semantic.foreground,
          fontFamily: fontFamily,
          fontSize: OpenChatTypography.sectionTitle,
          fontWeight: FontWeight.w600,
          height: 28 / OpenChatTypography.sectionTitle,
        ),
        contentTextStyle: TextStyle(
          color: semantic.mutedForeground,
          fontFamily: fontFamily,
          fontSize: OpenChatTypography.body,
          height: 20 / OpenChatTypography.body,
        ),
      ),
      cardTheme: CardThemeData(
        color: semantic.surface,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.panel),
          side: BorderSide(color: semantic.border),
        ),
      ),
      tooltipTheme: TooltipThemeData(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
        decoration: BoxDecoration(
          color: semantic.elevatedSurface,
          border: Border.all(color: semantic.border),
          borderRadius: BorderRadius.circular(OpenChatRadii.control),
        ),
        textStyle: TextStyle(
          color: semantic.foreground,
          fontFamily: fontFamily,
          fontSize: OpenChatTypography.metadata,
          height: 16 / OpenChatTypography.metadata,
        ),
      ),
    );
  }

  static WidgetStateProperty<Color?> _controlOverlay(OpenChatPalette palette) {
    return WidgetStateProperty.resolveWith((states) {
      if (states.contains(WidgetState.disabled)) return Colors.transparent;
      if (states.contains(WidgetState.pressed)) {
        return palette.selected.withValues(alpha: 0.8);
      }
      if (states.contains(WidgetState.hovered) ||
          states.contains(WidgetState.focused)) {
        return palette.hover.withValues(alpha: 0.7);
      }
      return Colors.transparent;
    });
  }

  static WidgetStateProperty<Color?> _primaryOverlay(OpenChatPalette palette) {
    return WidgetStateProperty.resolveWith((states) {
      if (states.contains(WidgetState.disabled)) return Colors.transparent;
      if (states.contains(WidgetState.pressed)) {
        return palette.surface.withValues(alpha: 0.22);
      }
      if (states.contains(WidgetState.hovered) ||
          states.contains(WidgetState.focused)) {
        return palette.surface.withValues(alpha: 0.14);
      }
      return Colors.transparent;
    });
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
    final semantic =
        theme.extension<OpenChatSemanticColors>() ??
        OpenChatSemanticColors._(palette);
    return _create(palette, theme.brightness, fontFamily: fontFamily).copyWith(
      extensions: <ThemeExtension<dynamic>>[
        palette,
        semantic,
        OpenChatConversationStyle(maxWidth: maxWidth, fontFamily: fontFamily),
      ],
    );
  }
}
