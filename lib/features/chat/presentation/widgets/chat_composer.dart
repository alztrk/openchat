import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../../../app/zihora_theme.dart';
import '../../../../l10n/zihora_localizations.dart';
import '../../domain/chatgpt_connection.dart';

class ChatComposer extends StatelessWidget {
  const ChatComposer({
    required this.controller,
    required this.onSendMessage,
    required this.canSendMessage,
    required this.showReasoningSelector,
    this.models = const <ChatGptModel>[],
    this.selectedModelId,
    this.onModelSelected,
    this.reasoningOptions = const <String>[],
    this.onReasoningSelected,
    this.isSending = false,
    this.onStopMessage,
    this.modelLabel,
    this.reasoningLevel,
    super.key,
  });

  final TextEditingController controller;
  final VoidCallback onSendMessage;
  final bool canSendMessage;
  final bool showReasoningSelector;
  final List<ChatGptModel> models;
  final String? selectedModelId;
  final ValueChanged<String>? onModelSelected;
  final List<String> reasoningOptions;
  final ValueChanged<String>? onReasoningSelected;
  final bool isSending;
  final VoidCallback? onStopMessage;
  final String? modelLabel;
  final String? reasoningLevel;

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);

    return LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < 900;

        return ConstrainedBox(
          constraints: BoxConstraints(minHeight: compact ? 116 : 100),
          child: Container(
            decoration: BoxDecoration(
              color: palette.composer,
              border: Border.all(color: palette.controlBorder),
              borderRadius: BorderRadius.circular(compact ? 12 : 16),
            ),
            padding: EdgeInsets.fromLTRB(
              compact ? 15 : 18,
              compact ? 11 : 16,
              compact ? 15 : 18,
              14,
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Focus(
                  onKeyEvent: (node, event) {
                    if (isSending &&
                        event is KeyDownEvent &&
                        event.logicalKey == LogicalKeyboardKey.escape) {
                      onStopMessage?.call();
                      return KeyEventResult.handled;
                    }
                    if (event is KeyDownEvent &&
                        (event.logicalKey == LogicalKeyboardKey.enter ||
                            event.logicalKey ==
                                LogicalKeyboardKey.numpadEnter) &&
                        !HardwareKeyboard.instance.isShiftPressed) {
                      if (canSendMessage && controller.text.trim().isNotEmpty) {
                        onSendMessage();
                      }
                      return KeyEventResult.handled;
                    }
                    return KeyEventResult.ignored;
                  },
                  child: TextField(
                    controller: controller,
                    minLines: 1,
                    maxLines: 3,
                    textCapitalization: TextCapitalization.sentences,
                    textInputAction: TextInputAction.newline,
                    decoration: InputDecoration(
                      hintText: context.zihoraL10n.messageHint,
                      hintStyle: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 15,
                        fontWeight: FontWeight.w400,
                        height: 20 / 15,
                      ),
                      filled: false,
                      border: InputBorder.none,
                      enabledBorder: InputBorder.none,
                      focusedBorder: InputBorder.none,
                      contentPadding: EdgeInsets.zero,
                      isDense: true,
                      isCollapsed: true,
                    ),
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 15,
                      fontWeight: FontWeight.w400,
                      height: 20 / 15,
                    ),
                  ),
                ),
                SizedBox(height: compact ? 32 : 12),
                _ComposerActions(
                  availableWidth: constraints.maxWidth,
                  controller: controller,
                  onSendMessage: onSendMessage,
                  canSendMessage: canSendMessage,
                  showReasoningSelector: showReasoningSelector,
                  models: models,
                  selectedModelId: selectedModelId,
                  onModelSelected: onModelSelected,
                  reasoningOptions: reasoningOptions,
                  onReasoningSelected: onReasoningSelected,
                  isSending: isSending,
                  onStopMessage: onStopMessage,
                  modelLabel: modelLabel,
                  reasoningLevel: reasoningLevel,
                  compact: compact,
                  palette: palette,
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _ComposerActions extends StatelessWidget {
  const _ComposerActions({
    required this.availableWidth,
    required this.controller,
    required this.onSendMessage,
    required this.canSendMessage,
    required this.showReasoningSelector,
    required this.models,
    required this.selectedModelId,
    required this.onModelSelected,
    required this.reasoningOptions,
    required this.onReasoningSelected,
    required this.isSending,
    required this.onStopMessage,
    required this.modelLabel,
    required this.reasoningLevel,
    required this.compact,
    required this.palette,
  });

  final double availableWidth;
  final TextEditingController controller;
  final VoidCallback onSendMessage;
  final bool canSendMessage;
  final bool showReasoningSelector;
  final List<ChatGptModel> models;
  final String? selectedModelId;
  final ValueChanged<String>? onModelSelected;
  final List<String> reasoningOptions;
  final ValueChanged<String>? onReasoningSelected;
  final bool isSending;
  final VoidCallback? onStopMessage;
  final String? modelLabel;
  final String? reasoningLevel;
  final bool compact;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.zihoraL10n;
    final dark = Theme.of(context).brightness == Brightness.dark;
    final iconRoot = dark ? 'assets/icons/dark' : 'assets/icons';
    final selectedModelLabel = modelLabel?.trim();
    final modelSelector = _ModelSelector(
      label: selectedModelLabel == null || selectedModelLabel.isEmpty
          ? l10n.modelSelection
          : selectedModelLabel,
      iconRoot: iconRoot,
      palette: palette,
      compact: compact,
      models: models,
      selectedModelId: selectedModelId,
      onSelected: onModelSelected,
    );
    final reasoningSelector = showReasoningSelector
        ? _ReasoningSelector(
            label: l10n.reasoning,
            level: reasoningLevel ?? l10n.reasoningMedium,
            iconRoot: iconRoot,
            palette: palette,
            unavailableHint: l10n.reasoningUnavailable,
            compact: compact,
            options: reasoningOptions,
            onSelected: onReasoningSelected,
          )
        : null;
    final leftControls = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        modelSelector,
        if (reasoningSelector != null) ...[
          const SizedBox(width: 12),
          reasoningSelector,
        ],
      ],
    );

    final trailingControls = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Tooltip(
          message: l10n.attachmentsUnavailable,
          child: OutlinedButton(
            onPressed: null,
            style: _controlStyle(
              palette,
              width: 36,
              compact: false,
              sideColor: palette.controlBorder,
            ),
            child: SvgPicture.asset(
              '$iconRoot/attachment.svg',
              width: 18,
              height: 18,
              colorFilter: ColorFilter.mode(
                palette.secondaryText,
                BlendMode.srcIn,
              ),
              excludeFromSemantics: true,
            ),
          ),
        ),
        const SizedBox(width: 8),
        ValueListenableBuilder<TextEditingValue>(
          valueListenable: controller,
          builder: (context, value, _) {
            final canAttemptSend =
                canSendMessage && value.text.trim().isNotEmpty;
            final width = showReasoningSelector ? 96.0 : 84.0;

            return SizedBox(
              width: width,
              height: 36,
              child: FilledButton(
                onPressed: isSending
                    ? onStopMessage
                    : canAttemptSend
                    ? onSendMessage
                    : null,
                style: FilledButton.styleFrom(
                  backgroundColor: palette.selected,
                  foregroundColor: palette.text,
                  disabledBackgroundColor: palette.selected,
                  disabledForegroundColor: palette.secondaryText,
                  tapTargetSize: compact
                      ? MaterialTapTargetSize.padded
                      : MaterialTapTargetSize.shrinkWrap,
                  padding: EdgeInsets.zero,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(9),
                  ),
                  textStyle: const TextStyle(
                    fontFamily: 'Manrope',
                    fontSize: 13,
                    fontWeight: FontWeight.w500,
                    height: 18 / 13,
                  ),
                ),
                child: isSending
                    ? Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          const Icon(Icons.stop_rounded, size: 16),
                          const SizedBox(width: 6),
                          Text(l10n.stop),
                        ],
                      )
                    : showReasoningSelector
                    ? Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          SvgPicture.asset(
                            '$iconRoot/send.svg',
                            width: 16,
                            height: 16,
                            colorFilter: ColorFilter.mode(
                              canAttemptSend
                                  ? palette.text
                                  : palette.secondaryText,
                              BlendMode.srcIn,
                            ),
                            excludeFromSemantics: true,
                          ),
                          const SizedBox(width: 6),
                          Flexible(
                            child: Text(
                              l10n.send,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        ],
                      )
                    : Text(l10n.send),
              ),
            );
          },
        ),
      ],
    );

    if (availableWidth < 520) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            spacing: 12,
            runSpacing: 8,
            children: [modelSelector, ?reasoningSelector],
          ),
          const SizedBox(height: 8),
          Align(alignment: Alignment.centerRight, child: trailingControls),
        ],
      );
    }

    return Row(
      children: [
        leftControls,
        if (availableWidth >= 820) ...[
          SizedBox(width: compact ? 16 : 12),
          Expanded(
            child: Text(
              l10n.keyboardHint,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: palette.secondaryText,
                fontSize: 12,
                fontWeight: FontWeight.w400,
                height: 18 / 12,
              ),
            ),
          ),
        ] else
          const Spacer(),
        trailingControls,
      ],
    );
  }
}

class _ModelSelector extends StatelessWidget {
  const _ModelSelector({
    required this.label,
    required this.iconRoot,
    required this.palette,
    required this.compact,
    required this.models,
    required this.selectedModelId,
    required this.onSelected,
  });

  final String label;
  final String iconRoot;
  final ZihoraPalette palette;
  final bool compact;
  final List<ChatGptModel> models;
  final String? selectedModelId;
  final ValueChanged<String>? onSelected;

  @override
  Widget build(BuildContext context) {
    final availableModels = models
        .where((model) => model.isAvailable)
        .toList(growable: false);
    final hasSelectedModel = availableModels.any(
      (model) => model.id == selectedModelId,
    );
    final screenWidth = MediaQuery.sizeOf(context).width;
    final menuWidth = math.max(0.0, math.min(520.0, screenWidth - 32));
    final providerWidth = math.min(
      menuWidth * 0.36,
      menuWidth < 420 ? 112.0 : 152.0,
    );
    final modelsHeight = math.min(availableModels.length * 44.0, 360.0);
    final menuHeight = math.max(modelsHeight, 88.0);

    return MenuAnchor(
      crossAxisUnconstrained: true,
      alignmentOffset: const Offset(0, 8),
      reservedPadding: const EdgeInsets.all(12),
      style: MenuStyle(
        backgroundColor: WidgetStatePropertyAll(palette.surface),
        elevation: const WidgetStatePropertyAll(8),
        padding: const WidgetStatePropertyAll(EdgeInsets.zero),
        side: WidgetStatePropertyAll(BorderSide(color: palette.border)),
        shape: WidgetStatePropertyAll(
          RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
        maximumSize: WidgetStatePropertyAll(Size(menuWidth, 420)),
      ),
      menuChildren: [
        SizedBox(
          width: menuWidth,
          height: menuHeight,
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Container(
                width: providerWidth,
                color: palette.navigation,
                padding: const EdgeInsets.all(8),
                child: Semantics(
                  selected: true,
                  child: Container(
                    decoration: BoxDecoration(
                      color: palette.selected,
                      borderRadius: BorderRadius.circular(8),
                    ),
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 12,
                    ),
                    child: Row(
                      children: [
                        SvgPicture.asset(
                          'assets/icons/chatgpt.svg',
                          width: 18,
                          height: 18,
                          colorFilter: ColorFilter.mode(
                            palette.secondaryIcon,
                            BlendMode.srcIn,
                          ),
                          excludeFromSemantics: true,
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            context.zihoraL10n.chatGptProvider,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: palette.text,
                              fontSize: 13,
                              fontWeight: FontWeight.w500,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
              VerticalDivider(width: 1, thickness: 1, color: palette.border),
              Expanded(
                child: SizedBox(
                  height: menuHeight,
                  child: ListView.builder(
                    padding: const EdgeInsets.symmetric(vertical: 4),
                    itemCount: availableModels.length,
                    itemExtent: 44,
                    itemBuilder: (context, index) {
                      final model = availableModels[index];
                      return MenuItemButton(
                        onPressed: onSelected == null
                            ? null
                            : () => onSelected!(model.id),
                        leadingIcon: model.id == selectedModelId
                            ? const Icon(Icons.check_rounded, size: 18)
                            : const SizedBox(width: 18, height: 18),
                        child: Text(
                          model.displayName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      );
                    },
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
      builder: (context, controller, _) => SizedBox(
        width: 132,
        height: 36,
        child: OutlinedButton(
          onPressed: availableModels.isEmpty || onSelected == null
              ? null
              : () =>
                    controller.isOpen ? controller.close() : controller.open(),
          style: _controlStyle(
            palette,
            width: 132,
            compact: compact,
            leftPadding: 12,
            rightPadding: 12,
          ),
          child: Row(
            children: [
              SizedBox(
                width: 16,
                height: 16,
                child: Center(
                  child: hasSelectedModel
                      ? SvgPicture.asset(
                          'assets/icons/chatgpt.svg',
                          width: 16,
                          height: 16,
                          colorFilter: ColorFilter.mode(
                            palette.secondaryIcon,
                            BlendMode.srcIn,
                          ),
                          excludeFromSemantics: true,
                        )
                      : SvgPicture.asset(
                          '$iconRoot/model.svg',
                          width: 12,
                          height: 12,
                          excludeFromSemantics: true,
                        ),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  textAlign: TextAlign.left,
                  style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w500,
                    height: 20 / 14,
                  ),
                ),
              ),
              SizedBox(
                width: 16,
                height: 16,
                child: Center(
                  child: SvgPicture.asset(
                    '$iconRoot/chevron.svg',
                    width: 10.6667,
                    height: 6.66668,
                    excludeFromSemantics: true,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ReasoningSelector extends StatelessWidget {
  const _ReasoningSelector({
    required this.label,
    required this.level,
    required this.iconRoot,
    required this.palette,
    required this.unavailableHint,
    required this.compact,
    required this.options,
    required this.onSelected,
  });

  final String label;
  final String level;
  final String iconRoot;
  final ZihoraPalette palette;
  final String unavailableHint;
  final bool compact;
  final List<String> options;
  final ValueChanged<String>? onSelected;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: options.isEmpty ? unavailableHint : label,
      child: MenuAnchor(
        menuChildren: [
          for (final option in options)
            MenuItemButton(
              onPressed: onSelected == null ? null : () => onSelected!(option),
              child: Text(_reasoningLabel(context, option)),
            ),
        ],
        builder: (context, controller, _) => SizedBox(
          width: 176,
          height: 36,
          child: OutlinedButton(
            onPressed: options.isEmpty || onSelected == null
                ? null
                : () => controller.isOpen
                      ? controller.close()
                      : controller.open(),
            style: _controlStyle(palette, width: 176, compact: compact),
            child: Row(
              children: [
                SvgPicture.asset(
                  '$iconRoot/brain.svg',
                  width: 16,
                  height: 16,
                  colorFilter: ColorFilter.mode(
                    palette.secondaryText,
                    BlendMode.srcIn,
                  ),
                  excludeFromSemantics: true,
                ),
                const SizedBox(width: 4),
                Expanded(
                  child: Row(
                    children: [
                      Flexible(
                        child: Text(
                          '$label ·',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            color: palette.secondaryText,
                            fontSize: 11,
                            fontWeight: FontWeight.w400,
                            height: 16 / 11,
                          ),
                        ),
                      ),
                      const SizedBox(width: 4),
                      Text(
                        _reasoningLabel(context, level),
                        style: TextStyle(
                          color: palette.text,
                          fontSize: 12,
                          fontWeight: FontWeight.w500,
                          height: 16 / 12,
                        ),
                      ),
                    ],
                  ),
                ),
                SizedBox(
                  width: 16,
                  height: 16,
                  child: Center(
                    child: SvgPicture.asset(
                      '$iconRoot/chevron.svg',
                      width: 10.6667,
                      height: 6.66668,
                      colorFilter: ColorFilter.mode(
                        palette.secondaryText,
                        BlendMode.srcIn,
                      ),
                      excludeFromSemantics: true,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

String _reasoningLabel(BuildContext context, String value) {
  final l10n = context.zihoraL10n;
  return switch (value) {
    'minimal' => l10n.reasoningMinimal,
    'low' => l10n.reasoningLow,
    'medium' => l10n.reasoningMedium,
    'high' => l10n.reasoningHigh,
    'xhigh' => l10n.reasoningExtraHigh,
    'max' => l10n.reasoningMax,
    'ultra' => l10n.reasoningUltra,
    _ => value,
  };
}

ButtonStyle _controlStyle(
  ZihoraPalette palette, {
  required double width,
  bool compact = false,
  double leftPadding = 10,
  double rightPadding = 10,
  Color? sideColor,
}) {
  return OutlinedButton.styleFrom(
    minimumSize: Size(width, 36),
    maximumSize: Size(width, 36),
    padding: EdgeInsets.fromLTRB(leftPadding, 0, rightPadding, 0),
    visualDensity: VisualDensity.standard,
    foregroundColor: palette.text,
    disabledForegroundColor: palette.secondaryText,
    tapTargetSize: compact
        ? MaterialTapTargetSize.padded
        : MaterialTapTargetSize.shrinkWrap,
    side: BorderSide(color: sideColor ?? palette.border),
    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(9)),
    textStyle: const TextStyle(
      fontFamily: 'Manrope',
      fontSize: 13,
      fontWeight: FontWeight.w500,
      height: 18 / 13,
    ),
  );
}
