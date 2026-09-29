import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../../../app/openchat_select.dart';
import '../../../../app/openchat_theme.dart';
import 'composer_control_style.dart';
import 'model_selector.dart';
import '../../../../l10n/openchat_localizations.dart';
import '../../../settings/data/settings_preferences.dart';
import '../../domain/chatgpt_connection.dart';
import '../../domain/model_favorite.dart';

class ChatComposer extends StatelessWidget {
  const ChatComposer({
    required this.controller,
    required this.onSendMessage,
    required this.canSendMessage,
    required this.showReasoningSelector,
    this.toolPermissionMode = ToolPermissionMode.requireApproval,
    this.onToolPermissionModeChanged,
    this.isLoadingModels = false,
    this.models = const <ChatGptModel>[],
    this.favoriteModels = const <FavoriteModel>[],
    this.providerId = 'chatgpt',
    this.onProviderSelected,
    this.modelsEmptyLabel,
    this.selectedModelId,
    this.onModelSelected,
    this.onModelFavoriteChanged,
    this.onFavoriteModelSelected,
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
  final ToolPermissionMode toolPermissionMode;
  final ValueChanged<ToolPermissionMode>? onToolPermissionModeChanged;
  final bool isLoadingModels;
  final List<ChatGptModel> models;
  final List<FavoriteModel> favoriteModels;
  final String providerId;
  final ValueChanged<String>? onProviderSelected;
  final String? modelsEmptyLabel;
  final String? selectedModelId;
  final ValueChanged<String>? onModelSelected;
  final void Function(
    String providerId,
    String modelId,
    String displayName,
    bool isFavorite,
  )?
  onModelFavoriteChanged;
  final ValueChanged<FavoriteModel>? onFavoriteModelSelected;
  final List<String> reasoningOptions;
  final ValueChanged<String>? onReasoningSelected;
  final bool isSending;
  final VoidCallback? onStopMessage;
  final String? modelLabel;
  final String? reasoningLevel;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);

    return LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < 900;

        return ConstrainedBox(
          constraints: BoxConstraints(minHeight: compact ? 104 : 100),
          child: Container(
            decoration: BoxDecoration(
              color: palette.composer,
              border: Border.all(color: palette.border),
              borderRadius: BorderRadius.circular(compact ? 16 : 20),
            ),
            padding: EdgeInsets.fromLTRB(
              compact ? 15 : 18,
              compact ? 12 : 16,
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
                      hintText: context.openchatL10n.messageHint,
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
                SizedBox(height: compact ? 16 : 12),
                _ComposerActions(
                  availableWidth: constraints.maxWidth,
                  controller: controller,
                  onSendMessage: onSendMessage,
                  canSendMessage: canSendMessage,
                  showReasoningSelector: showReasoningSelector,
                  toolPermissionMode: toolPermissionMode,
                  onToolPermissionModeChanged: onToolPermissionModeChanged,
                  isLoadingModels: isLoadingModels,
                  models: models,
                  favoriteModels: favoriteModels,
                  providerId: providerId,
                  onProviderSelected: onProviderSelected,
                  modelsEmptyLabel: modelsEmptyLabel,
                  selectedModelId: selectedModelId,
                  onModelSelected: onModelSelected,
                  onModelFavoriteChanged: onModelFavoriteChanged,
                  onFavoriteModelSelected: onFavoriteModelSelected,
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
    this.toolPermissionMode = ToolPermissionMode.requireApproval,
    this.onToolPermissionModeChanged,
    required this.isLoadingModels,
    required this.models,
    required this.favoriteModels,
    required this.providerId,
    required this.onProviderSelected,
    required this.modelsEmptyLabel,
    required this.selectedModelId,
    required this.onModelSelected,
    required this.onModelFavoriteChanged,
    required this.onFavoriteModelSelected,
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
  final ToolPermissionMode toolPermissionMode;
  final ValueChanged<ToolPermissionMode>? onToolPermissionModeChanged;
  final bool isLoadingModels;
  final List<ChatGptModel> models;
  final List<FavoriteModel> favoriteModels;
  final String providerId;
  final ValueChanged<String>? onProviderSelected;
  final String? modelsEmptyLabel;
  final String? selectedModelId;
  final ValueChanged<String>? onModelSelected;
  final void Function(
    String providerId,
    String modelId,
    String displayName,
    bool isFavorite,
  )?
  onModelFavoriteChanged;
  final ValueChanged<FavoriteModel>? onFavoriteModelSelected;
  final List<String> reasoningOptions;
  final ValueChanged<String>? onReasoningSelected;
  final bool isSending;
  final VoidCallback? onStopMessage;
  final String? modelLabel;
  final String? reasoningLevel;
  final bool compact;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final dark = Theme.of(context).brightness == Brightness.dark;
    final iconRoot = dark ? 'assets/icons/dark' : 'assets/icons';
    final selectedModelLabel = modelLabel?.trim();
    final modelSelector = ModelSelector(
      label: selectedModelLabel == null || selectedModelLabel.isEmpty
          ? l10n.modelSelection
          : selectedModelLabel,
      iconRoot: iconRoot,
      palette: palette,
      compact: compact,
      models: models,
      favoriteModels: favoriteModels,
      selectedModelId: selectedModelId,
      providerId: providerId,
      onProviderSelected: onProviderSelected,
      isLoadingModels: isLoadingModels,
      emptyModelsLabel:
          modelsEmptyLabel ?? context.openchatL10n.noModelsAvailable,
      onSelected: onModelSelected,
      onFavoriteChanged: onModelFavoriteChanged,
      onFavoriteSelected: onFavoriteModelSelected,
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
    final toolPermissionSelector = _ToolPermissionSelector(
      mode: toolPermissionMode,
      iconRoot: iconRoot,
      palette: palette,
      compact: compact,
      onSelected: onToolPermissionModeChanged,
    );
    final leftControls = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        modelSelector,
        if (reasoningSelector != null) ...[
          const SizedBox(width: 12),
          reasoningSelector,
        ],
        const SizedBox(width: 12),
        toolPermissionSelector,
      ],
    );

    final trailingControls = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Tooltip(
          message: l10n.attachmentsUnavailable,
          child: OutlinedButton(
            onPressed: null,
            style: composerControlStyle(
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

    if (availableWidth < 720) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            spacing: 12,
            runSpacing: 8,
            children: [
              modelSelector,
              ?reasoningSelector,
              toolPermissionSelector,
            ],
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
                color: palette.text,
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
  final OpenChatPalette palette;
  final String unavailableHint;
  final bool compact;
  final List<String> options;
  final ValueChanged<String>? onSelected;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: options.isEmpty ? unavailableHint : label,
      child: OpenChatSelect<String>(
        options: [
          for (final option in options)
            OpenChatSelectOption<String>(
              value: option,
              label: _reasoningLabel(context, option),
            ),
        ],
        value: level,
        onChanged: onSelected,
        palette: palette,
        width: 176,
        menuWidth: 176,
        height: 36,
        compact: compact,
        triggerStyle: composerControlStyle(
          palette,
          width: 176,
          compact: compact,
        ),
        trailingContent: _composerChevron(iconRoot, palette),
        trailingGap: 0,
        selectedContent: Row(
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
    );
  }
}

class _ToolPermissionSelector extends StatelessWidget {
  const _ToolPermissionSelector({
    required this.mode,
    required this.iconRoot,
    required this.palette,
    required this.compact,
    required this.onSelected,
  });

  final ToolPermissionMode mode;
  final String iconRoot;
  final OpenChatPalette palette;
  final bool compact;
  final ValueChanged<ToolPermissionMode>? onSelected;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final label = switch (mode) {
      ToolPermissionMode.requireApproval => l10n.toolPermissionRequireApproval,
      ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccess,
    };
    return OpenChatSelect<ToolPermissionMode>(
      options: [
        for (final option in ToolPermissionMode.values)
          OpenChatSelectOption<ToolPermissionMode>(
            value: option,
            label: switch (option) {
              ToolPermissionMode.requireApproval =>
                l10n.toolPermissionRequireApproval,
              ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccess,
            },
            icon: switch (option) {
              ToolPermissionMode.requireApproval =>
                Icons.admin_panel_settings_outlined,
              ToolPermissionMode.fullAccess => Icons.gpp_maybe_outlined,
            },
            iconColor: option == ToolPermissionMode.fullAccess
                ? palette.accentIcon
                : palette.secondaryText,
          ),
      ],
      value: mode,
      onChanged: onSelected,
      palette: palette,
      width: 148,
      menuWidth: 176,
      height: 36,
      compact: compact,
      triggerStyle: composerControlStyle(
        palette,
        width: 148,
        compact: compact,
        leftPadding: 10,
        rightPadding: 10,
      ),
      leadingIcon: Icons.admin_panel_settings_outlined,
      leadingIconColor: palette.secondaryText,
      leadingIconGap: 5,
      selectedContent: Text(
        label,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: TextStyle(
          color: palette.text,
          fontSize: 12,
          fontWeight: FontWeight.w500,
          height: 16 / 12,
        ),
      ),
      trailingContent: _composerChevron(iconRoot, palette),
      trailingGap: 0,
    );
  }
}

Widget _composerChevron(String iconRoot, OpenChatPalette palette) {
  return SizedBox(
    width: 16,
    height: 16,
    child: Center(
      child: SvgPicture.asset(
        '$iconRoot/chevron.svg',
        width: 10.6667,
        height: 6.66668,
        colorFilter: ColorFilter.mode(palette.secondaryText, BlendMode.srcIn),
        excludeFromSemantics: true,
      ),
    ),
  );
}

String _reasoningLabel(BuildContext context, String value) {
  final l10n = context.openchatL10n;
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
