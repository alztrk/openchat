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

bool _usesTouchTargets(BuildContext context) {
  return switch (Theme.of(context).platform) {
    TargetPlatform.android ||
    TargetPlatform.iOS ||
    TargetPlatform.fuchsia => true,
    TargetPlatform.windows ||
    TargetPlatform.macOS ||
    TargetPlatform.linux => false,
  };
}

Color _focusRingColor(BuildContext context) {
  return OpenChatSemanticColors.of(context).focusRing;
}

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
        final narrow = constraints.maxWidth <= 600;
        final touchTargets = _usesTouchTargets(context);
        final focusRing = _focusRingColor(context);
        final horizontalPadding = narrow ? 12.0 : 16.0;

        return ConstrainedBox(
          constraints: BoxConstraints(minHeight: narrow ? 104 : 100),
          child: Focus(
            skipTraversal: true,
            child: Builder(
              builder: (focusContext) {
                final hasFocus = Focus.of(focusContext).hasFocus;
                return Container(
                  decoration: BoxDecoration(
                    color: palette.composer,
                    border: Border.all(
                      color: hasFocus
                          ? focusRing
                          : palette.border.withValues(alpha: 0.72),
                      width: hasFocus ? 1.4 : 1,
                    ),
                    borderRadius: BorderRadius.circular(narrow ? 16 : 20),
                  ),
                  padding: EdgeInsets.fromLTRB(
                    horizontalPadding,
                    narrow ? 12 : 15,
                    horizontalPadding,
                    narrow ? 12 : 13,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Focus(
                        skipTraversal: true,
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
                          cursorColor: focusRing,
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
                            color: palette.text,
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
                      SizedBox(height: narrow ? 14 : 12),
                      _ComposerActions(
                        controller: controller,
                        onSendMessage: onSendMessage,
                        canSendMessage: canSendMessage,
                        showReasoningSelector: showReasoningSelector,
                        toolPermissionMode: toolPermissionMode,
                        onToolPermissionModeChanged:
                            onToolPermissionModeChanged,
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
                        conversationMemoryRepository:
                            conversationMemoryRepository,
                        settingsPreferences: settingsPreferences,
                        pendingAttachments: pendingAttachments,
                        onAddAttachments: onAddAttachments,
                        attachmentsEnabled: attachmentsEnabled,
                        narrow: narrow,
                        touchTargets: touchTargets,
                        palette: palette,
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
        );
      },
    );
  }
}

class _ComposerActions extends StatelessWidget {
  const _ComposerActions({
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
    required this.narrow,
    required this.touchTargets,
    required this.palette,
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
  final bool attachmentsEnabled;
  final bool narrow;
  final bool touchTargets;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final dark = Theme.of(context).brightness == Brightness.dark;
    final iconRoot = dark ? 'assets/icons/dark' : 'assets/icons';
    final focusRing = _focusRingColor(context);
    final semantic = OpenChatSemanticColors.of(context);
    final primary = semantic.primary;
    final primaryForeground = semantic.primaryForeground;
    final selectedModelLabel = modelLabel?.trim();
    final modelSelector = ModelSelector(
      label: selectedModelLabel == null || selectedModelLabel.isEmpty
          ? l10n.modelSelection
          : selectedModelLabel,
      iconRoot: iconRoot,
      palette: palette,
      compact: touchTargets,
      width: narrow ? 144 : 152,
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
            compact: touchTargets,
            width: narrow ? 144 : 152,
            options: reasoningOptions,
            onSelected: onReasoningSelected,
          )
        : null;
    final toolPermissionSelector = _ToolPermissionSelector(
      mode: toolPermissionMode,
      palette: palette,
      compact: touchTargets,
      width: narrow ? 128 : 136,
      hasSelectedModel: selectedModelId != null,
      supportsToolCalls: contextSupportsTools,
      onSelected: onToolPermissionModeChanged,
    );
    final attachmentSize = touchTargets ? 44.0 : 36.0;
    final sendSize = touchTargets ? 44.0 : 40.0;
    final attachmentButton = Tooltip(
      message: attachmentsEnabled
          ? l10n.attachFile
          : l10n.attachmentsUnavailable,
      child: Semantics(
        button: true,
        enabled: attachmentsEnabled,
        label: attachmentsEnabled
            ? l10n.attachFile
            : l10n.attachmentsUnavailable,
        child: SizedBox.square(
          dimension: attachmentSize,
          child: OutlinedButton(
            onPressed: attachmentsEnabled ? onAddAttachments : null,
            style: composerControlStyle(
              palette,
              width: attachmentSize,
              height: attachmentSize,
              compact: touchTargets,
              focusColor: focusRing,
            ),
            child: Icon(
              Icons.attach_file_rounded,
              size: 18,
              color: attachmentsEnabled
                  ? palette.secondaryIcon
                  : palette.disabledIcon,
            ),
          ),
        ),
      ),
    );
    final contextIndicator = ContextUsageIndicator(
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
    );
    final sendButton = ValueListenableBuilder<TextEditingValue>(
      valueListenable: controller,
      builder: (context, value, _) {
        final canAttemptSend =
            canSendMessage &&
            (value.text.trim().isNotEmpty || pendingAttachments.isNotEmpty);
        final actionEnabled = isSending || canAttemptSend;
        final onPressed = isSending
            ? onStopMessage
            : canAttemptSend
            ? () {
                if (controller.text.trim().isNotEmpty ||
                    pendingAttachments.isNotEmpty) {
                  onSendMessage();
                }
              }
            : null;
        final tooltip = isSending ? l10n.stop : l10n.send;
        return Tooltip(
          message: tooltip,
          child: Semantics(
            button: true,
            enabled: actionEnabled,
            label: tooltip,
            child: ExcludeSemantics(
              child: SizedBox.square(
                dimension: sendSize,
                child: IconButton(
                  onPressed: onPressed,
                  tooltip: null,
                  padding: EdgeInsets.zero,
                  iconSize: 18,
                  constraints: BoxConstraints.tightFor(
                    width: sendSize,
                    height: sendSize,
                  ),
                  style: IconButton.styleFrom(
                    backgroundColor: actionEnabled ? primary : palette.selected,
                    foregroundColor: actionEnabled
                        ? primaryForeground
                        : palette.disabledForeground,
                    disabledBackgroundColor: palette.selected,
                    disabledForegroundColor: palette.disabledForeground,
                    minimumSize: Size.square(sendSize),
                    maximumSize: Size.square(sendSize),
                    padding: EdgeInsets.zero,
                    tapTargetSize: touchTargets
                        ? MaterialTapTargetSize.padded
                        : MaterialTapTargetSize.shrinkWrap,
                    shape: const CircleBorder(),
                  ),
                  icon: isSending
                      ? const Icon(Icons.stop_rounded)
                      : SvgPicture.asset(
                          '$iconRoot/send.svg',
                          width: 18,
                          height: 18,
                          colorFilter: ColorFilter.mode(
                            actionEnabled
                                ? primaryForeground
                                : palette.disabledForeground,
                            BlendMode.srcIn,
                          ),
                          excludeFromSemantics: true,
                        ),
                ),
              ),
            ),
          ),
        );
      },
    );
    final leftControls = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        attachmentButton,
        const SizedBox(width: 8),
        toolPermissionSelector,
      ],
    );
    final trailingControls = Row(
      mainAxisSize: MainAxisSize.min,
      children: [contextIndicator, const SizedBox(width: 8), sendButton],
    );
    final rightControls = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        modelSelector,
        if (reasoningSelector != null) ...[
          const SizedBox(width: 8),
          reasoningSelector,
        ],
        const SizedBox(width: 8),
        trailingControls,
      ],
    );

    final narrowControls = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [attachmentButton, toolPermissionSelector],
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [modelSelector, ?reasoningSelector],
        ),
        const SizedBox(height: 8),
        Align(alignment: Alignment.centerRight, child: trailingControls),
      ],
    );

    if (narrow) {
      return narrowControls;
    }

    return Row(children: [leftControls, const Spacer(), rightControls]);
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
    required this.width,
    required this.options,
    required this.onSelected,
  });

  final String label;
  final String? level;
  final String iconRoot;
  final OpenChatPalette palette;
  final String defaultHint;
  final bool compact;
  final double width;
  final List<String> options;
  final ValueChanged<String?>? onSelected;

  @override
  Widget build(BuildContext context) {
    final currentLevel = level;
    final l10n = context.openchatL10n;
    return Tooltip(
      message:
          '$label: ${currentLevel == null ? defaultHint : _reasoningLabel(context, currentLevel)}',
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
        width: width,
        menuWidth: width,
        height: composerControlHeight(context),
        compact: compact,
        triggerStyle: composerControlStyle(
          palette,
          width: width,
          height: composerControlHeight(context),
          compact: compact,
          focusColor: _focusRingColor(context),
        ),
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
            Flexible(
              child: Text(
                currentLevel == null
                    ? l10n.reasoningDefault
                    : _reasoningLabel(context, currentLevel),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: palette.text,
                  fontSize: 12,
                  fontWeight: FontWeight.w500,
                  height: 16 / 12,
                ),
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
    required this.palette,
    required this.compact,
    required this.width,
    required this.hasSelectedModel,
    required this.supportsToolCalls,
    required this.onSelected,
  });

  final ToolPermissionMode mode;
  final OpenChatPalette palette;
  final bool compact;
  final double width;
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
              ToolPermissionMode.requireApproval => Icons.lock_outline_rounded,
              ToolPermissionMode.fullAccess => Icons.lock_open_rounded,
            },
            iconColor: option == ToolPermissionMode.fullAccess
                ? palette.accentIcon
                : palette.secondaryText,
          ),
      ],
      value: mode,
      onChanged: onSelected,
      palette: palette,
      width: width,
      menuWidth: width,
      height: composerControlHeight(context),
      compact: compact,
      triggerStyle: composerControlStyle(
        palette,
        width: width,
        height: composerControlHeight(context),
        compact: compact,
        leftPadding: 10,
        rightPadding: 10,
        focusColor: _focusRingColor(context),
      ),
      leadingIcon: mode == ToolPermissionMode.fullAccess
          ? Icons.lock_open_rounded
          : Icons.lock_outline_rounded,
      leadingIconColor: palette.secondaryText,
      leadingIconGap: 5,
      selectedContent: Text(
        label,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: Theme.of(context).textTheme.labelLarge?.copyWith(
          color: palette.text,
          fontSize: 13,
          fontWeight: FontWeight.w500,
          height: 18 / 13,
        ),
      ),
    );
    final capabilityNotice = !hasSelectedModel
        ? null
        : switch (supportsToolCalls) {
            true => null,
            false => l10n.selectedModelDoesNotSupportToolCalls,
            null => l10n.selectedModelToolSupportUnknown,
          };
    return Tooltip(
      message: capabilityNotice == null ? label : '$label\n$capabilityNotice',
      child: selector,
    );
  }
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
