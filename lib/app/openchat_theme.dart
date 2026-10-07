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
      Color.alphaBlend(text.withValues(alpha: 0.56), surface);
  Color get disabledIcon => disabledForeground;
  Color get disabledSurface =>
      Color.alphaBlend(text.withValues(alpha: 0.045), surface);
  Color get disabledBorder =>
      Color.alphaBlend(border.withValues(alpha: 0.58), surface);

  static const light = OpenChatPalette(
    navigation: Color(0xFFF0F0F0),
    surface: Color(0xFFFAFAFA),
    composer: Color(0xFFF0F0F0),
    selected: Color(0xFFE4E4E4),
    hover: Color(0xFFE9E9E9),
    border: Color(0xFFD9D9D9),
    controlBorder: Color(0xFF8B8B8B),
    text: Color(0xFF232323),
    brandInk: Color(0xFF232323),
    secondaryText: Color(0xFF636363),
    secondaryIcon: Color(0xFF707070),
    accent: Color(0xFF2563EB),
    accentIcon: Color(0xFF2563EB),
  );

  static const dark = OpenChatPalette(
    navigation: Color(0xFF1B1C1D),
    surface: Color(0xFF181818),
    composer: Color(0xFF343434),
    selected: Color(0xFF2B2C2D),
    hover: Color(0xFF252627),
    border: Color(0xFF303133),
    controlBorder: Color(0xFF777777),
    text: Color(0xFFEDEDED),
    brandInk: Color(0xFFEDEDED),
    secondaryText: Color(0xFFA0A0A0),
    secondaryIcon: Color(0xFF939393),
    accent: Color(0xFF70A1FF),
    accentIcon: Color(0xFF70A1FF),
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

/// Semantic colors shared by controls, surfaces, status states, and focus.
///
/// Keep these roles independent from component-specific palette names so a
/// component can use the same visual language in both light and dark themes.
class OpenChatSemanticColors extends ThemeExtension<OpenChatSemanticColors> {
  const OpenChatSemanticColors({
    required this.background,
    required this.foreground,
    required this.muted,
    required this.mutedForeground,
    required this.surface,
    required this.elevatedSurface,
    required this.border,
    required this.subtleBorder,
    required this.primary,
    required this.primaryHover,
    required this.primaryActive,
    required this.primaryForeground,
    required this.secondary,
    required this.accent,
    required this.destructive,
    required this.warning,
    required this.success,
    required this.info,
    required this.focusRing,
  });

  final Color background;
  final Color foreground;
  final Color muted;
  final Color mutedForeground;
  final Color surface;
  final Color elevatedSurface;
  final Color border;
  final Color subtleBorder;
  final Color primary;
  final Color primaryHover;
  final Color primaryActive;
  final Color primaryForeground;
  final Color secondary;
  final Color accent;
  final Color destructive;
  final Color warning;
  final Color success;
  final Color info;
  final Color focusRing;

  static const light = OpenChatSemanticColors(
    background: Color(0xFFF0F0F0),
    foreground: Color(0xFF232323),
    muted: Color(0xFFE9E9E9),
    mutedForeground: Color(0xFF636363),
    surface: Color(0xFFFAFAFA),
    elevatedSurface: Color(0xFFF7F7F7),
    border: Color(0xFFD9D9D9),
    subtleBorder: Color(0xFFE4E4E4),
    primary: Color(0xFF2563EB),
    primaryHover: Color(0xFF1D4ED8),
    primaryActive: Color(0xFF1E40AF),
    primaryForeground: Color(0xFFFFFFFF),
    secondary: Color(0xFFE4E4E4),
    accent: Color(0xFF3B82F6),
    destructive: Color(0xFFC62828),
    warning: Color(0xFFB45309),
    success: Color(0xFF16794A),
    info: Color(0xFF0369A1),
    focusRing: Color(0xFF2563EB),
  );

  static const dark = OpenChatSemanticColors(
    background: Color(0xFF1E1F21),
    foreground: Color(0xFFEDEDED),
    muted: Color(0xFF252627),
    mutedForeground: Color(0xFFA0A0A0),
    surface: Color(0xFF181818),
    elevatedSurface: Color(0xFF292A2B),
    border: Color(0xFF38393A),
    subtleBorder: Color(0xFF303133),
    primary: Color(0xFF70A1FF),
    primaryHover: Color(0xFF93B8FF),
    primaryActive: Color(0xFFB0CAFF),
    primaryForeground: Color(0xFF071426),
    secondary: Color(0xFF2B2C2D),
    accent: Color(0xFF5B9BFF),
    destructive: Color(0xFFF38B8B),
    warning: Color(0xFFF6C177),
    success: Color(0xFF71D6A1),
    info: Color(0xFF7CC4FF),
    focusRing: Color(0xFF8BB5FF),
  );

  static OpenChatSemanticColors of(BuildContext context) {
    final colors = Theme.of(context).extension<OpenChatSemanticColors>();
    if (colors == null) {
      throw StateError(
        'OpenChatSemanticColors is missing from the active theme.',
      );
    }
    return colors;
  }

  @override
  OpenChatSemanticColors copyWith({
    Color? background,
    Color? foreground,
    Color? muted,
    Color? mutedForeground,
    Color? surface,
    Color? elevatedSurface,
    Color? border,
    Color? subtleBorder,
    Color? primary,
    Color? primaryHover,
    Color? primaryActive,
    Color? primaryForeground,
    Color? secondary,
    Color? accent,
    Color? destructive,
    Color? warning,
    Color? success,
    Color? info,
    Color? focusRing,
  }) {
    return OpenChatSemanticColors(
      background: background ?? this.background,
      foreground: foreground ?? this.foreground,
      muted: muted ?? this.muted,
      mutedForeground: mutedForeground ?? this.mutedForeground,
      surface: surface ?? this.surface,
      elevatedSurface: elevatedSurface ?? this.elevatedSurface,
      border: border ?? this.border,
      subtleBorder: subtleBorder ?? this.subtleBorder,
      primary: primary ?? this.primary,
      primaryHover: primaryHover ?? this.primaryHover,
      primaryActive: primaryActive ?? this.primaryActive,
      primaryForeground: primaryForeground ?? this.primaryForeground,
      secondary: secondary ?? this.secondary,
      accent: accent ?? this.accent,
      destructive: destructive ?? this.destructive,
      warning: warning ?? this.warning,
      success: success ?? this.success,
      info: info ?? this.info,
      focusRing: focusRing ?? this.focusRing,
    );
  }

  @override
  OpenChatSemanticColors lerp(
    ThemeExtension<OpenChatSemanticColors>? other,
    double t,
  ) {
    if (other is! OpenChatSemanticColors) return this;
    return OpenChatSemanticColors(
      background: Color.lerp(background, other.background, t) ?? background,
      foreground: Color.lerp(foreground, other.foreground, t) ?? foreground,
      muted: Color.lerp(muted, other.muted, t) ?? muted,
      mutedForeground:
          Color.lerp(mutedForeground, other.mutedForeground, t) ??
          mutedForeground,
      surface: Color.lerp(surface, other.surface, t) ?? surface,
      elevatedSurface:
          Color.lerp(elevatedSurface, other.elevatedSurface, t) ??
          elevatedSurface,
      border: Color.lerp(border, other.border, t) ?? border,
      subtleBorder:
          Color.lerp(subtleBorder, other.subtleBorder, t) ?? subtleBorder,
      primary: Color.lerp(primary, other.primary, t) ?? primary,
      primaryHover:
          Color.lerp(primaryHover, other.primaryHover, t) ?? primaryHover,
      primaryActive:
          Color.lerp(primaryActive, other.primaryActive, t) ?? primaryActive,
      primaryForeground:
          Color.lerp(primaryForeground, other.primaryForeground, t) ??
          primaryForeground,
      secondary: Color.lerp(secondary, other.secondary, t) ?? secondary,
      accent: Color.lerp(accent, other.accent, t) ?? accent,
      destructive: Color.lerp(destructive, other.destructive, t) ?? destructive,
      warning: Color.lerp(warning, other.warning, t) ?? warning,
      success: Color.lerp(success, other.success, t) ?? success,
      info: Color.lerp(info, other.info, t) ?? info,
      focusRing: Color.lerp(focusRing, other.focusRing, t) ?? focusRing,
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
  static const menu = 10.0;
  static const card = 14.0;
  static const dialog = 16.0;
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
    final semantic = brightness == Brightness.light
        ? OpenChatSemanticColors.light
        : OpenChatSemanticColors.dark;
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
          fontSize: 24,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 1.3,
        ),
        titleLarge: TextStyle(
          color: semantic.foreground,
          fontSize: 18,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 1.2,
        ),
        titleMedium: TextStyle(
          color: semantic.foreground,
          fontSize: 16,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 1.4,
        ),
        bodyLarge: TextStyle(
          color: semantic.foreground,
          fontSize: 15,
          fontWeight: FontWeight.w500,
          letterSpacing: 0,
          height: 1.45,
        ),
        bodyMedium: TextStyle(
          color: semantic.mutedForeground,
          fontSize: 13,
          fontWeight: FontWeight.w500,
          letterSpacing: 0,
          height: 1.5,
        ),
        bodySmall: TextStyle(
          color: semantic.mutedForeground,
          fontSize: 12,
          fontWeight: FontWeight.w500,
          letterSpacing: 0,
          height: 1.45,
        ),
        labelLarge: TextStyle(
          color: semantic.foreground,
          fontSize: 13,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
          height: 1.4,
        ),
      ).apply(fontFamily: fontFamily),
      dividerColor: semantic.border,
      inputDecorationTheme: InputDecorationTheme(
        hintStyle: TextStyle(color: semantic.mutedForeground, fontSize: 13),
        labelStyle: TextStyle(color: semantic.mutedForeground, fontSize: 13),
        floatingLabelStyle: TextStyle(color: semantic.primary, fontSize: 13),
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
          minimumSize: const WidgetStatePropertyAll(Size(36, 36)),
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
          minimumSize: const WidgetStatePropertyAll(Size(36, 36)),
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
          minimumSize: const WidgetStatePropertyAll(Size(36, 36)),
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
              fontSize: 13,
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
          minimumSize: const WidgetStatePropertyAll(Size(36, 36)),
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
              fontSize: 13,
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
          minimumSize: const WidgetStatePropertyAll(Size(36, 36)),
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
          fontSize: 18,
          fontWeight: FontWeight.w600,
          height: 1.3,
        ),
        contentTextStyle: TextStyle(
          color: semantic.mutedForeground,
          fontFamily: fontFamily,
          fontSize: 13,
          height: 1.5,
        ),
      ),
      cardTheme: CardThemeData(
        color: semantic.surface,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
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
          fontSize: 12,
          height: 1.35,
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
        (theme.brightness == Brightness.light
            ? OpenChatSemanticColors.light
            : OpenChatSemanticColors.dark);
    return _create(palette, theme.brightness, fontFamily: fontFamily).copyWith(
      extensions: <ThemeExtension<dynamic>>[
        palette,
        semantic,
        OpenChatConversationStyle(maxWidth: maxWidth, fontFamily: fontFamily),
      ],
    );
  }
}
