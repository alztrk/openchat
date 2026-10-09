import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:shadcn_ui/shadcn_ui.dart' as shad;

import 'package:openchat/app/openchat_theme.dart';

class OpenChatSelectOption<T> {
  const OpenChatSelectOption({
    required this.value,
    required this.label,
    this.description,
    this.icon,
    this.iconColor,
    this.descriptionColor,
    this.selectedColor,
    this.textStyle,
    this.enabled = true,
  });

  final T value;
  final String label;
  final String? description;
  final IconData? icon;
  final Color? iconColor;
  final Color? descriptionColor;
  final Color? selectedColor;
  final TextStyle? textStyle;
  final bool enabled;
}

class OpenChatSelectField<T> extends StatelessWidget {
  const OpenChatSelectField({
    required this.label,
    required this.options,
    required this.value,
    required this.onChanged,
    required this.palette,
    this.height = 44,
    super.key,
  });

  final String label;
  final List<OpenChatSelectOption<T>> options;
  final T? value;
  final ValueChanged<T>? onChanged;
  final OpenChatPalette palette;
  final double height;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return MergeSemantics(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsetsDirectional.only(start: 2, bottom: 4),
            child: Text(
              label,
              style: theme.textTheme.bodySmall?.copyWith(
                color: palette.secondaryText,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
          LayoutBuilder(
            builder: (context, constraints) {
              final menuWidth = constraints.maxWidth.isFinite
                  ? constraints.maxWidth
                  : 240.0;
              return OpenChatSelect<T>(
                options: options,
                value: value,
                onChanged: onChanged,
                palette: palette,
                width: double.infinity,
                menuWidth: menuWidth,
                height: height,
              );
            },
          ),
        ],
      ),
    );
  }
}

class OpenChatSelect<T> extends StatelessWidget {
  const OpenChatSelect({
    required this.options,
    required this.value,
    required this.onChanged,
    required this.palette,
    this.width,
    this.menuWidth,
    this.height = 40,
    this.hint,
    this.leadingIcon,
    this.leadingIconColor,
    this.leadingIconGap = 8,
    this.selectedContent,
    this.trailingContent,
    this.trailingGap = 8,
    this.horizontalPadding = 12,
    this.borderless = false,
    super.key,
  });

  final List<OpenChatSelectOption<T>> options;
  final T? value;
  final ValueChanged<T>? onChanged;
  final OpenChatPalette palette;
  final double? width;
  final double? menuWidth;
  final double height;
  final String? hint;
  final IconData? leadingIcon;
  final Color? leadingIconColor;
  final double leadingIconGap;
  final Widget? selectedContent;
  final Widget? trailingContent;
  final double trailingGap;
  final double horizontalPadding;
  final bool borderless;

  OpenChatSelectOption<T>? _optionForValue(Object? value) {
    for (final option in options) {
      if (option.value == value) return option;
    }
    return null;
  }

  TextStyle _optionTextStyle(
    OpenChatSelectOption<T> option, {
    required bool selected,
  }) {
    final color = option.textStyle?.color ?? palette.text;
    return option.textStyle?.copyWith(
          color: option.enabled ? color : palette.disabledForeground,
          fontWeight: selected ? FontWeight.w600 : FontWeight.w500,
        ) ??
        TextStyle(
          color: option.enabled ? palette.text : palette.disabledForeground,
          fontSize: 13,
          fontWeight: selected ? FontWeight.w600 : FontWeight.w500,
        );
  }

  Widget _optionContents(OpenChatSelectOption<T> option) {
    return Row(
      children: [
        if (option.icon != null) ...[
          Icon(
            option.icon,
            size: 16,
            color: option.enabled
                ? option.iconColor ?? palette.secondaryIcon
                : palette.disabledIcon,
          ),
          const SizedBox(width: 10),
        ],
        Expanded(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                option.label,
                style: _optionTextStyle(option, selected: false),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              if (option.description case final description?) ...[
                const SizedBox(height: 2),
                Text(
                  description,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: option.enabled
                        ? option.descriptionColor ?? palette.secondaryText
                        : palette.disabledForeground,
                    fontSize: 12,
                    fontWeight: FontWeight.w400,
                    height: 16 / 12,
                  ),
                ),
              ],
            ],
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final isEnabled = onChanged != null && options.isNotEmpty;
    final selected = _optionForValue(value);
    final selectedValue = selected == null
        ? null
        : _OpenChatSelectValue<T>(selected.value);
    final availableWidth = math.max(0.0, MediaQuery.sizeOf(context).width - 32);
    final requestedMenuWidth =
        menuWidth ?? (width != null && width!.isFinite ? width! : 240);
    final resolvedMenuWidth = math.min(requestedMenuWidth, availableWidth);
    final mediaQuery = MediaQuery.of(context);
    final availableHeight = math.max(
      0.0,
      mediaQuery.size.height -
          mediaQuery.viewInsets.bottom -
          mediaQuery.viewPadding.vertical,
    );
    final maxMenuHeight = math.min(384.0, availableHeight * 0.5);
    final placeholderText = hint ?? '';
    final triggerDecoration = shad.ShadDecoration(
      color: Colors.transparent,
      border: borderless
          ? shad.ShadBorder.none
          : shad.ShadBorder.all(
              color: isEnabled ? palette.controlBorder : palette.disabledBorder,
              radius: BorderRadius.circular(OpenChatRadii.control),
            ),
      focusedBorder: shad.ShadBorder.all(
        color: palette.focusRing,
        width: 2,
        radius: BorderRadius.circular(OpenChatRadii.control),
      ),
    );

    return MergeSemantics(
      child: Semantics(
        button: true,
        enabled: isEnabled,
        label: selected?.label ?? placeholderText,
        hint: selected?.description,
        child: SizedBox(
          width: width,
          height: math.max(
            height,
            MediaQuery.textScalerOf(context).scale(18) + 12,
          ),
          child: shad.ShadSelect<_OpenChatSelectValue<T>>(
            enabled: isEnabled,
            initialValue: selectedValue,
            onChanged: (newValue) {
              if (newValue != null) onChanged?.call(newValue.value);
            },
            options: [
              for (final option in options)
                if (option.enabled)
                  shad.ShadOption<_OpenChatSelectValue<T>>(
                    value: _OpenChatSelectValue<T>(option.value),
                    padding: EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: option.description == null ? 10 : 8,
                    ),
                    hoveredBackgroundColor: palette.hover,
                    selectedBackgroundColor: palette.hover,
                    textStyle: _optionTextStyle(option, selected: false),
                    selectedTextStyle: _optionTextStyle(option, selected: true),
                    selectedIcon: Padding(
                      padding: const EdgeInsets.only(right: 8),
                      child: Icon(
                        LucideIcons.check,
                        size: 16,
                        color: option.selectedColor ?? palette.accentIcon,
                      ),
                    ),
                    child: ConstrainedBox(
                      constraints: BoxConstraints(
                        minHeight: option.description == null ? 44 : 58,
                        maxWidth: resolvedMenuWidth,
                      ),
                      child: _optionContents(option),
                    ),
                  )
                else
                  Semantics(
                    button: true,
                    enabled: false,
                    label: option.label,
                    hint: option.description,
                    child: ExcludeSemantics(
                      child: ConstrainedBox(
                        constraints: BoxConstraints(
                          minHeight: option.description == null ? 44 : 58,
                          maxWidth: resolvedMenuWidth,
                        ),
                        child: Padding(
                          padding: EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: option.description == null ? 10 : 8,
                          ),
                          child: _optionContents(option),
                        ),
                      ),
                    ),
                  ),
            ],
            selectedOptionBuilder: (context, selectedValue) {
              final selectedOption = _optionForValue(selectedValue.value);
              if (selectedOption == null) {
                return Text(
                  placeholderText,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: 13,
                    fontWeight: FontWeight.w500,
                  ),
                );
              }
              final customContent = selectedContent;
              if (customContent != null) {
                return Opacity(
                  opacity: isEnabled ? 1 : 0.55,
                  child: customContent,
                );
              }
              return Text(
                selectedOption.label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: _optionTextStyle(selectedOption, selected: true)
                    .copyWith(
                      color: isEnabled
                          ? selectedOption.textStyle?.color ?? palette.text
                          : palette.disabledForeground,
                      fontWeight: FontWeight.w500,
                    ),
              );
            },
            placeholder: Text(
              placeholderText,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: isEnabled
                    ? palette.secondaryText
                    : palette.disabledForeground,
                fontSize: 13,
                fontWeight: FontWeight.w500,
              ),
            ),
            placeholderStyle: TextStyle(
              color: palette.secondaryText,
              fontSize: 13,
              fontWeight: FontWeight.w500,
            ),
            trailing: trailingContent == null
                ? Icon(
                    LucideIcons.chevronDown,
                    size: 18,
                    color: isEnabled
                        ? palette.secondaryIcon
                        : palette.disabledIcon,
                  )
                : Padding(
                    padding: EdgeInsetsDirectional.only(start: trailingGap),
                    child: Opacity(
                      opacity: isEnabled ? 1 : 0.55,
                      child: trailingContent,
                    ),
                  ),
            padding: EdgeInsets.symmetric(horizontal: horizontalPadding),
            optionsPadding: const EdgeInsets.all(4),
            minWidth: 0,
            maxWidth: resolvedMenuWidth,
            maxHeight: maxMenuHeight,
            decoration: triggerDecoration,
          ),
        ),
      ),
    );
  }
}

@immutable
class _OpenChatSelectValue<T> {
  const _OpenChatSelectValue(this.value);

  final T value;

  @override
  bool operator ==(Object other) =>
      other is _OpenChatSelectValue<T> && other.value == value;

  @override
  int get hashCode => value.hashCode;
}
