import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_dropdown.dart';
import 'package:openchat/app/openchat_theme.dart';

class OpenChatSelectOption<T> {
  const OpenChatSelectOption({
    required this.value,
    required this.label,
    this.icon,
    this.iconColor,
    this.textStyle,
    this.enabled = true,
  });

  final T value;
  final String label;
  final IconData? icon;
  final Color? iconColor;
  final TextStyle? textStyle;
  final bool enabled;
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
          MenuItemButton(
            onPressed: option.enabled && onChanged != null
                ? () => onChanged!(option.value)
                : null,
            style: OpenChatDropdown.menuItemStyle(palette),
            child: ConstrainedBox(
              constraints: BoxConstraints(
                minHeight: 44,
                maxWidth: resolvedMenuWidth,
              ),
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: option.value == value
                      ? palette.hover
                      : option.enabled
                      ? Colors.transparent
                      : palette.disabledSurface,
                  borderRadius: BorderRadius.circular(7),
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 10,
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
                        child: Text(
                          option.label,
                          style:
                              option.textStyle?.copyWith(
                                color: option.enabled
                                    ? option.textStyle?.color ?? palette.text
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
                        ),
                      ),
                      if (option.value == value)
                        Icon(
                          Icons.check_rounded,
                          size: 16,
                          color: palette.accentIcon,
                        ),
                    ],
                  ),
                ),
              ),
            ),
          ),
      ],
      builder: (context, controller, _) => SizedBox(
        width: width,
        height: math.max(
          height,
          MediaQuery.textScalerOf(context).scale(18) + 12,
        ),
        child: OutlinedButton(
          onPressed: onChanged == null || options.isEmpty
              ? null
              : () =>
                    controller.isOpen ? controller.close() : controller.open(),
          style:
              triggerStyle ??
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
                side: BorderSide(
                  color: isEnabled ? palette.border : palette.disabledBorder,
                ),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(OpenChatRadii.control),
                ),
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
                      ? Icons.keyboard_arrow_up_rounded
                      : Icons.keyboard_arrow_down_rounded,
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
