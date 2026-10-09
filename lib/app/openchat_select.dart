import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_dropdown.dart';
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
    this.compact = false,
    this.hint,
    this.leadingIcon,
    this.leadingIconColor,
    this.leadingIconGap = 8,
    this.selectedContent,
    this.trailingContent,
    this.trailingGap = 8,
    this.triggerStyle,
    super.key,
  });

  final List<OpenChatSelectOption<T>> options;
  final T? value;
  final ValueChanged<T>? onChanged;
  final OpenChatPalette palette;
  final double? width;
  final double? menuWidth;
  final double height;
  final bool compact;
  final String? hint;
  final IconData? leadingIcon;
  final Color? leadingIconColor;
  final double leadingIconGap;
  final Widget? selectedContent;
  final Widget? trailingContent;
  final double trailingGap;
  final ButtonStyle? triggerStyle;

  @override
  Widget build(BuildContext context) {
    final isEnabled = onChanged != null && options.isNotEmpty;
    final selectedOption = options.where((option) => option.value == value);
    final selected = selectedOption.isEmpty ? null : selectedOption.first;
    final selectedTextStyle = selected?.textStyle;
    final availableWidth = math.max(0.0, MediaQuery.sizeOf(context).width - 32);
    final requestedMenuWidth =
        menuWidth ?? (width != null && width!.isFinite ? width! : 240);
    final resolvedMenuWidth = math.min(requestedMenuWidth, availableWidth);
    final trailing = trailingContent;

    return OpenChatDropdown(
      palette: palette,
      alignmentOffset: const Offset(0, 6),
      menuChildren: [
        for (final option in options)
          Semantics(
            button: true,
            enabled: option.enabled && onChanged != null,
            selected: option.value == value,
            label: option.label,
            hint: option.description,
            onTap: option.enabled && onChanged != null
                ? () => onChanged!(option.value)
                : null,
            child: ExcludeSemantics(
              child: MenuItemButton(
                onPressed: option.enabled && onChanged != null
                    ? () => onChanged!(option.value)
                    : null,
                style: OpenChatDropdown.menuItemStyle(palette),
                child: ConstrainedBox(
                  constraints: BoxConstraints(
                    minHeight: option.description == null ? 44 : 58,
                    maxWidth: resolvedMenuWidth,
                  ),
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: option.value == value
                          ? palette.hover
                          : option.enabled
                          ? Colors.transparent
                          : palette.disabledSurface,
                      borderRadius: BorderRadius.circular(
                        OpenChatRadii.control,
                      ),
                    ),
                    child: Padding(
                      padding: EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: option.description == null ? 10 : 8,
                      ),
                      child: Row(
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
                                  style:
                                      option.textStyle?.copyWith(
                                        color: option.enabled
                                            ? option.textStyle?.color ??
                                                  palette.text
                                            : palette.disabledForeground,
                                        fontWeight: option.value == value
                                            ? FontWeight.w600
                                            : FontWeight.w500,
                                      ) ??
                                      TextStyle(
                                        color: option.enabled
                                            ? palette.text
                                            : palette.disabledForeground,
                                        fontSize: 13,
                                        fontWeight: option.value == value
                                            ? FontWeight.w600
                                            : FontWeight.w500,
                                      ),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                ),
                                if (option.description
                                    case final description?) ...[
                                  const SizedBox(height: 2),
                                  Text(
                                    description,
                                    maxLines: 2,
                                    overflow: TextOverflow.ellipsis,
                                    style: TextStyle(
                                      color: option.enabled
                                          ? option.descriptionColor ??
                                                palette.secondaryText
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
                          if (option.value == value)
                            Icon(
                              LucideIcons.check,
                              size: 16,
                              color: option.selectedColor ?? palette.accentIcon,
                            ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
      ],
      builder: (context, controller, _, focusNode) => SizedBox(
        width: width,
        height: math.max(
          height,
          MediaQuery.textScalerOf(context).scale(18) + 12,
        ),
        child: OutlinedButton(
          focusNode: focusNode,
          onPressed: onChanged == null || options.isEmpty
              ? null
              : () =>
                    controller.isOpen ? controller.close() : controller.open(),
          style:
              (triggerStyle ??
                      OutlinedButton.styleFrom(
                        minimumSize: Size(0, height),
                        padding: const EdgeInsets.symmetric(horizontal: 12),
                        visualDensity: VisualDensity.standard,
                        tapTargetSize: compact
                            ? MaterialTapTargetSize.padded
                            : MaterialTapTargetSize.shrinkWrap,
                        foregroundColor: palette.text,
                        disabledForegroundColor: palette.disabledForeground,
                        disabledBackgroundColor: palette.disabledSurface,
                        shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(
                            OpenChatRadii.control,
                          ),
                        ),
                      ))
                  .copyWith(
                    side: WidgetStateProperty.resolveWith<BorderSide>((states) {
                      if (!isEnabled) {
                        return BorderSide(color: palette.disabledBorder);
                      }
                      return BorderSide(
                        color: states.contains(WidgetState.focused)
                            ? palette.focusRing
                            : palette.controlBorder,
                        width: states.contains(WidgetState.focused) ? 2 : 1,
                      );
                    }),
                  ),
          child: Row(
            children: [
              if (leadingIcon != null) ...[
                Icon(
                  leadingIcon,
                  size: 16,
                  color: isEnabled
                      ? leadingIconColor ?? palette.secondaryIcon
                      : palette.disabledIcon,
                ),
                SizedBox(width: leadingIconGap),
              ],
              Expanded(
                child: selectedContent == null
                    ? Text(
                        selected?.label ?? hint ?? '',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: selectedTextStyle == null
                            ? TextStyle(
                                color: !isEnabled
                                    ? palette.disabledForeground
                                    : selected == null
                                    ? palette.secondaryText
                                    : palette.text,
                                fontSize: 13,
                                fontWeight: FontWeight.w500,
                              )
                            : selectedTextStyle.copyWith(
                                color: !isEnabled
                                    ? palette.disabledForeground
                                    : selectedTextStyle.color ?? palette.text,
                                fontWeight: FontWeight.w500,
                              ),
                      )
                    : Opacity(
                        opacity: isEnabled ? 1 : 0.55,
                        child: selectedContent,
                      ),
              ),
              if (trailing != null) ...[
                SizedBox(width: trailingGap),
                Opacity(opacity: isEnabled ? 1 : 0.55, child: trailing),
              ] else ...[
                const SizedBox(width: 8),
                Icon(
                  controller.isOpen
                      ? LucideIcons.chevronUp
                      : LucideIcons.chevronDown,
                  size: 18,
                  color: isEnabled
                      ? palette.secondaryIcon
                      : palette.disabledIcon,
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
