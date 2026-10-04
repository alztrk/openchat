import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/conversation_memory_repository.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/domain/model_favorite.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

import 'package:openchat/features/chat/presentation/widgets/composer_control_style.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_attachment_gallery.dart';
import 'package:openchat/features/chat/presentation/widgets/context_usage_indicator.dart';
import 'package:openchat/features/chat/presentation/widgets/model_selector.dart';

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
    required this.providerId,
    this.isChatGptConnected = false,
    this.availableProviderIds = const <String>{},
    this.onProviderSelected,
    this.modelsEmptyLabel,
    this.hiddenModelKeys = const <String>{},
    this.selectedModelId,
    this.selectedModelRouteKey,
    this.onModelSelected,
    this.onModelFavoriteChanged,
    this.onFavoriteModelSelected,
    this.reasoningOptions = const <String>[],
    this.onReasoningSelected,
    this.isSending = false,
    this.onStopMessage,
    this.modelLabel,
    this.reasoningLevel,
    this.messages = const <ChatMessage>[],
    this.conversationId,
    this.contextProviderId,
    this.contextModelId,
    this.contextSupportsTools,
    this.contextWindow,
    this.contextConnectionId,
    this.contextWorkspaceId,
    this.conversationMemoryRepository,
    this.settingsPreferences,
    this.pendingAttachments = const <ChatAttachment>[],
    this.onAddAttachments,
    this.onRemoveAttachment,
    this.attachmentsEnabled = false,
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
  final bool isChatGptConnected;
  final Set<String> availableProviderIds;
  final ValueChanged<String>? onProviderSelected;
  final String? modelsEmptyLabel;
  final Set<String> hiddenModelKeys;
  final String? selectedModelId;
  final String? selectedModelRouteKey;
  final ValueChanged<ChatGptModel>? onModelSelected;
  final void Function(
    String providerId,
    String modelId,
    String displayName,
    String? sourceConnectionId,
    bool isFavorite,
  )?
  onModelFavoriteChanged;
  final ValueChanged<FavoriteModel>? onFavoriteModelSelected;
  final List<String> reasoningOptions;
  final ValueChanged<String?>? onReasoningSelected;
  final bool isSending;
  final VoidCallback? onStopMessage;
  final String? modelLabel;
  final String? reasoningLevel;
  final List<ChatMessage> messages;
  final String? conversationId;
  final String? contextProviderId;
  final String? contextModelId;
  final bool? contextSupportsTools;
  final int? contextWindow;
  final String? contextConnectionId;
  final String? contextWorkspaceId;
  final ConversationMemoryRepository? conversationMemoryRepository;
  final SettingsPreferences? settingsPreferences;
  final List<ChatAttachment> pendingAttachments;
  final VoidCallback? onAddAttachments;
  final ValueChanged<String>? onRemoveAttachment;
  final bool attachmentsEnabled;

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
                      if (canSendMessage &&
                          (controller.text.trim().isNotEmpty ||
                              pendingAttachments.isNotEmpty)) {
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
                if (pendingAttachments.isNotEmpty) ...[
                  const SizedBox(height: 12),
                  ChatAttachmentGallery(
                    attachments: pendingAttachments,
                    palette: palette,
                    preferredImageWidth: 160,
                    imageHeight: 104,
                    onRemoveAttachment: onRemoveAttachment,
                  ),
                ],
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
                  isChatGptConnected: isChatGptConnected,
                  availableProviderIds: availableProviderIds,
                  onProviderSelected: onProviderSelected,
                  modelsEmptyLabel: modelsEmptyLabel,
                  hiddenModelKeys: hiddenModelKeys,
                  selectedModelId: selectedModelId,
                  selectedModelRouteKey: selectedModelRouteKey,
                  onModelSelected: onModelSelected,
                  onModelFavoriteChanged: onModelFavoriteChanged,
                  onFavoriteModelSelected: onFavoriteModelSelected,
                  reasoningOptions: reasoningOptions,
                  onReasoningSelected: onReasoningSelected,
                  isSending: isSending,
                  onStopMessage: onStopMessage,
                  modelLabel: modelLabel,
                  reasoningLevel: reasoningLevel,
                  messages: messages,
                  conversationId: conversationId,
                  contextProviderId: contextProviderId,
                  contextModelId: contextModelId,
                  contextSupportsTools: contextSupportsTools,
                  contextWindow: contextWindow,
                  contextConnectionId: contextConnectionId,
                  contextWorkspaceId: contextWorkspaceId,
                  conversationMemoryRepository: conversationMemoryRepository,
                  settingsPreferences: settingsPreferences,
                  pendingAttachments: pendingAttachments,
                  onAddAttachments: onAddAttachments,
                  attachmentsEnabled: attachmentsEnabled,
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
    required this.isChatGptConnected,
    required this.availableProviderIds,
    required this.onProviderSelected,
    required this.modelsEmptyLabel,
    this.hiddenModelKeys = const <String>{},
    required this.selectedModelId,
    required this.selectedModelRouteKey,
    required this.onModelSelected,
    required this.onModelFavoriteChanged,
    required this.onFavoriteModelSelected,
    required this.reasoningOptions,
    required this.onReasoningSelected,
    required this.isSending,
    required this.onStopMessage,
    required this.modelLabel,
    required this.reasoningLevel,
    required this.messages,
    required this.conversationId,
    required this.contextProviderId,
    required this.contextModelId,
    required this.contextSupportsTools,
    required this.contextWindow,
    required this.contextConnectionId,
    required this.contextWorkspaceId,
    required this.conversationMemoryRepository,
    required this.settingsPreferences,
    required this.pendingAttachments,
    required this.onAddAttachments,
    required this.attachmentsEnabled,
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
  final bool isChatGptConnected;
  final Set<String> availableProviderIds;
  final ValueChanged<String>? onProviderSelected;
  final String? modelsEmptyLabel;
  final Set<String> hiddenModelKeys;
  final String? selectedModelId;
  final String? selectedModelRouteKey;
  final ValueChanged<ChatGptModel>? onModelSelected;
  final void Function(
    String providerId,
    String modelId,
    String displayName,
    String? sourceConnectionId,
    bool isFavorite,
  )?
  onModelFavoriteChanged;
  final ValueChanged<FavoriteModel>? onFavoriteModelSelected;
  final List<String> reasoningOptions;
  final ValueChanged<String?>? onReasoningSelected;
  final bool isSending;
  final VoidCallback? onStopMessage;
  final String? modelLabel;
  final String? reasoningLevel;
  final List<ChatMessage> messages;
  final String? conversationId;
  final String? contextProviderId;
  final String? contextModelId;
  final bool? contextSupportsTools;
  final int? contextWindow;
  final String? contextConnectionId;
  final String? contextWorkspaceId;
  final ConversationMemoryRepository? conversationMemoryRepository;
  final SettingsPreferences? settingsPreferences;
  final List<ChatAttachment> pendingAttachments;
  final VoidCallback? onAddAttachments;
  final bool attachmentsEnabled;
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
      selectedModelRouteKey: selectedModelRouteKey,
      providerId: providerId,
      isChatGptConnected: isChatGptConnected,
      availableProviderIds: availableProviderIds,
      onProviderSelected: onProviderSelected,
      isLoadingModels: isLoadingModels,
      emptyModelsLabel:
          modelsEmptyLabel ?? context.openchatL10n.noModelsAvailable,
      onSelected: onModelSelected,
      onFavoriteChanged: onModelFavoriteChanged,
      onFavoriteSelected: onFavoriteModelSelected,
      hiddenModelKeys: hiddenModelKeys,
    );
    final reasoningSelector = showReasoningSelector
        ? _ReasoningSelector(
            label: l10n.reasoning,
            level: reasoningLevel,
            iconRoot: iconRoot,
            palette: palette,
            defaultHint: l10n.reasoningDefaultHint,
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
      hasSelectedModel: selectedModelId != null,
      supportsToolCalls: contextSupportsTools,
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
        ContextUsageIndicator(
          controller: controller,
          messages: messages,
          providerId: contextProviderId,
          modelId: contextModelId,
          supportsTools: contextSupportsTools,
          contextWindow: contextWindow,
          repository: conversationMemoryRepository,
          conversationId: conversationId,
          isSending: isSending,
          connectionId: contextConnectionId,
          workspaceId: contextWorkspaceId,
          settingsPreferences: settingsPreferences,
          toolPermissionMode: toolPermissionMode,
          pendingAttachments: pendingAttachments,
        ),
        const SizedBox(width: 8),
        Tooltip(
          message: attachmentsEnabled
              ? l10n.attachFile
              : l10n.attachmentsUnavailable,
          child: OutlinedButton(
            onPressed: attachmentsEnabled ? onAddAttachments : null,
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
                attachmentsEnabled
                    ? palette.secondaryIcon
                    : palette.disabledIcon,
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
                canSendMessage &&
                (value.text.trim().isNotEmpty || pendingAttachments.isNotEmpty);
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
                  disabledBackgroundColor: palette.disabledSurface,
                  disabledForegroundColor: palette.disabledForeground,
                  tapTargetSize: compact
                      ? MaterialTapTargetSize.padded
                      : MaterialTapTargetSize.shrinkWrap,
                  padding: EdgeInsets.zero,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(9),
                  ),
                  textStyle: const TextStyle(
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
    required this.defaultHint,
    required this.compact,
    required this.options,
    required this.onSelected,
  });

  final String label;
  final String? level;
  final String iconRoot;
  final OpenChatPalette palette;
  final String defaultHint;
  final bool compact;
  final List<String> options;
  final ValueChanged<String?>? onSelected;

  @override
  Widget build(BuildContext context) {
    final currentLevel = level;
    final l10n = context.openchatL10n;
    return Tooltip(
      message: currentLevel == null ? defaultHint : label,
      child: OpenChatSelect<String?>(
        options: [
          OpenChatSelectOption<String?>(
            value: null,
            label: l10n.reasoningDefault,
          ),
          for (final option in options)
            OpenChatSelectOption<String?>(
              value: option,
              label: _reasoningLabel(context, option),
            ),
        ],
        value: currentLevel,
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
              currentLevel == null
                  ? l10n.reasoningDefault
                  : _reasoningLabel(context, currentLevel),
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
    required this.hasSelectedModel,
    required this.supportsToolCalls,
    required this.onSelected,
  });

  final ToolPermissionMode mode;
  final String iconRoot;
  final OpenChatPalette palette;
  final bool compact;
  final bool hasSelectedModel;
  final bool? supportsToolCalls;
  final ValueChanged<ToolPermissionMode>? onSelected;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final label = switch (mode) {
      ToolPermissionMode.requireApproval => l10n.toolPermissionRequireApproval,
      ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccess,
    };
    final selector = OpenChatSelect<ToolPermissionMode>(
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
    final capabilityNotice = !hasSelectedModel
        ? null
        : switch (supportsToolCalls) {
            true => null,
            false => l10n.selectedModelDoesNotSupportToolCalls,
            null => l10n.selectedModelToolSupportUnknown,
          };
    if (capabilityNotice == null) return selector;
    return Tooltip(message: capabilityNotice, child: selector);
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
