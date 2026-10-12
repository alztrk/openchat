import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:shadcn_ui/shadcn_ui.dart' as shad;

import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/conversation_memory_repository.dart';
import 'package:openchat/features/chat/data/model_pricing_repository.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/domain/model_favorite.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

import 'package:openchat/features/chat/presentation/widgets/composer_control_style.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_attachment_gallery.dart';
import 'package:openchat/features/chat/presentation/widgets/context_usage_indicator.dart';
import 'package:openchat/features/chat/presentation/widgets/model_selector.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';

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

enum ChatComposerMode { none, plan, goal }

class ChatComposer extends StatelessWidget {
  const ChatComposer({
    required this.controller,
    required this.onSendMessage,
    required this.canSendMessage,
    required this.showReasoningSelector,
    this.toolPermissionMode = ToolPermissionMode.requireApproval,
    this.onToolPermissionModeChanged,
    this.mode = ChatComposerMode.none,
    this.onModeChanged,
    this.onNewConversation,
    this.onNewConversationMessage,
    this.onOpenHistorySearch,
    this.onSummarizeConversation,
    this.onRenameCommand,
    this.onModelCommand,
    this.onReasoningCommand,
    this.onCompactCommand,
    this.isLoadingModels = false,
    this.models = const <ChatGptModel>[],
    this.slashModels = const <ChatGptModel>[],
    this.isLoadingSlashModels = false,
    this.onRefreshSlashModels,
    this.providerNames = const <String, String>{},
    this.favoriteModels = const <FavoriteModel>[],
    required this.providerId,
    this.isChatGptConnected = false,
    this.chatGptFastModeEnabled = false,
    this.chatGptFastModeAvailable = false,
    this.chatGptFastModeLoading = false,
    this.chatGptFastModeSaving = false,
    this.onChatGptFastModeChanged,
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
    this.modelPricingRepository,
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
  final ChatComposerMode mode;
  final ValueChanged<ChatComposerMode>? onModeChanged;
  final VoidCallback? onNewConversation;
  final ValueChanged<String>? onNewConversationMessage;
  final ValueChanged<String?>? onOpenHistorySearch;
  final ValueChanged<String?>? onSummarizeConversation;
  final ValueChanged<String>? onRenameCommand;
  final ValueChanged<String>? onModelCommand;
  final ValueChanged<String>? onReasoningCommand;
  final ValueChanged<String?>? onCompactCommand;
  final bool isLoadingModels;
  final List<ChatGptModel> models;
  final List<ChatGptModel> slashModels;
  final bool isLoadingSlashModels;
  final VoidCallback? onRefreshSlashModels;
  final Map<String, String> providerNames;
  final List<FavoriteModel> favoriteModels;
  final String providerId;
  final bool isChatGptConnected;
  final bool chatGptFastModeEnabled;
  final bool chatGptFastModeAvailable;
  final bool chatGptFastModeLoading;
  final bool chatGptFastModeSaving;
  final ValueChanged<bool>? onChatGptFastModeChanged;
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
  final ModelPricingRepository? modelPricingRepository;
  final SettingsPreferences? settingsPreferences;
  final List<ChatAttachment> pendingAttachments;
  final VoidCallback? onAddAttachments;
  final ValueChanged<String>? onRemoveAttachment;
  final bool attachmentsEnabled;

  void _sendMessageOrRunSummarizeCommand() {
    var rawText = controller.text.trim();
    if (mode != ChatComposerMode.none && rawText.startsWith('/')) {
      rawText = rawText.substring(1).trimLeft();
      controller.value = TextEditingValue(
        text: rawText,
        selection: TextSelection.collapsed(offset: rawText.length),
      );
      if (rawText.isEmpty && pendingAttachments.isEmpty) return;
    }
    final actionCommand = RegExp(
      r'^/(new|search)(?:\s+([\s\S]*))?$',
      caseSensitive: false,
    ).firstMatch(rawText);
    if (actionCommand != null) {
      final command = actionCommand.group(1)?.toLowerCase();
      final argument = actionCommand.group(2)?.trim() ?? '';
      controller.clear();
      switch (command) {
        case 'new':
          if (argument.isEmpty) {
            onNewConversation?.call();
          } else {
            onNewConversationMessage?.call(argument);
          }
        case 'search':
          onOpenHistorySearch?.call(argument.isEmpty ? null : argument);
      }
      return;
    }
    final argumentCommand = RegExp(
      r'^/(rename|model|reasoning)(?:\s+([\s\S]*))?$',
      caseSensitive: false,
    ).firstMatch(rawText);
    if (argumentCommand != null) {
      final command = argumentCommand.group(1)?.toLowerCase();
      final argument = argumentCommand.group(2)?.trim() ?? '';
      controller.clear();
      switch (command) {
        case 'rename':
          onRenameCommand?.call(argument);
        case 'model':
          onModelCommand?.call(argument);
        case 'reasoning':
          onReasoningCommand?.call(argument);
      }
      return;
    }
    final compactCommand = RegExp(
      r'^/compact(?:\s+([\s\S]*))?$',
      caseSensitive: false,
    ).firstMatch(rawText);
    if (compactCommand != null) {
      controller.clear();
      onCompactCommand?.call(compactCommand.group(1)?.trim());
      return;
    }
    final summarizeCommand = RegExp(
      r'^/summarize(?:\s|$)',
      caseSensitive: false,
    ).firstMatch(rawText);
    final onSummarize = onSummarizeConversation;
    if (summarizeCommand != null && onSummarize != null) {
      final instructions = rawText.substring(summarizeCommand.end).trim();
      controller.clear();
      onSummarize(instructions.isEmpty ? null : instructions);
      return;
    }
    onSendMessage();
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;

    return LayoutBuilder(
      builder: (context, constraints) {
        final narrow =
            constraints.maxWidth <= 600 ||
            MediaQuery.textScalerOf(context).scale(16) > 18;
        final touchTargets = _usesTouchTargets(context);
        final focusRing = _focusRingColor(context);
        final horizontalPadding = narrow ? 12.0 : 16.0;
        final reducedMotion = MediaQuery.disableAnimationsOf(context);

        return ConstrainedBox(
          constraints: BoxConstraints(minHeight: narrow ? 104 : 100),
          child: Focus(
            skipTraversal: true,
            child: Builder(
              builder: (focusContext) {
                final hasFocus = Focus.of(focusContext).hasFocus;
                return _SlashCommandInput(
                  controller: controller,
                  onSendMessage: _sendMessageOrRunSummarizeCommand,
                  canSendMessage: canSendMessage,
                  isSending: isSending,
                  onStopMessage: onStopMessage,
                  hasPendingAttachments: pendingAttachments.isNotEmpty,
                  palette: palette,
                  menuHorizontalPadding: horizontalPadding + (hasFocus ? 2 : 1),
                  onModeChanged: onModeChanged,
                  onNewConversation: onNewConversation,
                  onNewConversationMessage: onNewConversationMessage,
                  onOpenHistorySearch: onOpenHistorySearch,
                  onSummarizeConversation: onSummarizeConversation,
                  onCompactCommand: onCompactCommand,
                  slashModels: slashModels,
                  isLoadingSlashModels: isLoadingSlashModels,
                  onRefreshSlashModels: onRefreshSlashModels,
                  providerNames: providerNames,
                  hiddenModelKeys: hiddenModelKeys,
                  onModelSelected: onModelSelected,
                  child: AnimatedContainer(
                    duration: reducedMotion
                        ? Duration.zero
                        : const Duration(milliseconds: 180),
                    curve: Curves.easeOutCubic,
                    decoration: BoxDecoration(
                      color: palette.composer,
                      border: Border.all(
                        color: hasFocus ? focusRing : palette.controlBorder,
                        width: hasFocus ? 2 : 1,
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
                        MergeSemantics(
                          child: Semantics(
                            label: l10n.messageHint,
                            child: shad.ShadInput(
                              controller: controller,
                              minLines: 1,
                              maxLines: 3,
                              textCapitalization: TextCapitalization.sentences,
                              textInputAction: TextInputAction.newline,
                              cursorColor: focusRing,
                              decoration: shad.ShadDecoration.none,
                              padding: EdgeInsets.zero,
                              inputPadding: EdgeInsets.zero,
                              placeholder: ExcludeSemantics(
                                child: Text(l10n.messageHint),
                              ),
                              placeholderStyle: TextStyle(
                                color: palette.secondaryText,
                                fontSize: 15,
                                fontWeight: FontWeight.w400,
                                height: 20 / 15,
                              ),
                              style: TextStyle(
                                color: palette.text,
                                fontSize: 15,
                                fontWeight: FontWeight.w400,
                                height: 20 / 15,
                              ),
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
                          onSendMessage: _sendMessageOrRunSummarizeCommand,
                          canSendMessage: canSendMessage,
                          showReasoningSelector: showReasoningSelector,
                          toolPermissionMode: toolPermissionMode,
                          onToolPermissionModeChanged:
                              onToolPermissionModeChanged,
                          mode: mode,
                          onModeChanged: onModeChanged,
                          isLoadingModels: isLoadingModels,
                          models: models,
                          favoriteModels: favoriteModels,
                          providerId: providerId,
                          isChatGptConnected: isChatGptConnected,
                          chatGptFastModeEnabled: chatGptFastModeEnabled,
                          chatGptFastModeAvailable: chatGptFastModeAvailable,
                          chatGptFastModeLoading: chatGptFastModeLoading,
                          chatGptFastModeSaving: chatGptFastModeSaving,
                          onChatGptFastModeChanged: onChatGptFastModeChanged,
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
                          modelPricingRepository: modelPricingRepository,
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

class _SlashCommandInput extends StatefulWidget {
  const _SlashCommandInput({
    required this.controller,
    required this.onSendMessage,
    required this.canSendMessage,
    required this.isSending,
    required this.onStopMessage,
    required this.hasPendingAttachments,
    required this.palette,
    required this.menuHorizontalPadding,
    required this.onModeChanged,
    required this.onNewConversation,
    required this.onNewConversationMessage,
    required this.onOpenHistorySearch,
    required this.onSummarizeConversation,
    required this.onCompactCommand,
    required this.slashModels,
    required this.isLoadingSlashModels,
    required this.onRefreshSlashModels,
    required this.providerNames,
    required this.hiddenModelKeys,
    required this.onModelSelected,
    required this.child,
  });

  final TextEditingController controller;
  final VoidCallback onSendMessage;
  final bool canSendMessage;
  final bool isSending;
  final VoidCallback? onStopMessage;
  final bool hasPendingAttachments;
  final OpenChatPalette palette;
  final double menuHorizontalPadding;
  final ValueChanged<ChatComposerMode>? onModeChanged;
  final VoidCallback? onNewConversation;
  final ValueChanged<String>? onNewConversationMessage;
  final ValueChanged<String?>? onOpenHistorySearch;
  final ValueChanged<String?>? onSummarizeConversation;
  final ValueChanged<String?>? onCompactCommand;
  final List<ChatGptModel> slashModels;
  final bool isLoadingSlashModels;
  final VoidCallback? onRefreshSlashModels;
  final Map<String, String> providerNames;
  final Set<String> hiddenModelKeys;
  final ValueChanged<ChatGptModel>? onModelSelected;
  final Widget child;

  @override
  State<_SlashCommandInput> createState() => _SlashCommandInputState();
}

enum _ComposerSlashCommand {
  newConversation('/new'),
  search('/search'),
  summarize('/summarize'),
  rename('/rename'),
  model('/model'),
  compact('/compact'),
  reasoning('/reasoning'),
  plan('/plan'),
  goal('/goal');

  const _ComposerSlashCommand(this.name);

  final String name;
}

class _SlashCommandInputState extends State<_SlashCommandInput> {
  List<_ComposerSlashCommand> _matchingCommands = const [];
  int _selectedCommandIndex = 0;
  bool _showingModelPicker = false;
  bool _suppressCommandSuggestions = false;
  int _selectedModelIndex = 0;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_handleTextChanged);
    _matchingCommands = _findMatchingCommands(widget.controller.text);
  }

  @override
  void didUpdateWidget(covariant _SlashCommandInput oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_handleTextChanged);
      widget.controller.addListener(_handleTextChanged);
      _updateCommandSuggestions();
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_handleTextChanged);
    super.dispose();
  }

  List<_ComposerSlashCommand> _findMatchingCommands(String value) {
    if (_suppressCommandSuggestions ||
        !value.startsWith('/') ||
        value.contains(RegExp(r'\s'))) {
      return const [];
    }
    final normalized = value.toLowerCase();
    return _ComposerSlashCommand.values
        .where(
          (command) =>
              command.name.startsWith(normalized) ||
              (command == _ComposerSlashCommand.goal &&
                  '/hedef'.startsWith(normalized)),
        )
        .toList(growable: false);
  }

  void _handleTextChanged() => _updateCommandSuggestions();

  void _updateCommandSuggestions() {
    if (widget.controller.text.isEmpty) {
      _suppressCommandSuggestions = false;
    }
    final matches = _findMatchingCommands(widget.controller.text);
    final sameMatches =
        matches.length == _matchingCommands.length &&
        matches.indexed.every(
          (entry) => entry.$2 == _matchingCommands[entry.$1],
        );
    if (sameMatches || !mounted) return;
    setState(() {
      if (!_suppressCommandSuggestions) {
        _showingModelPicker = false;
      }
      _matchingCommands = matches;
      _selectedCommandIndex = 0;
      _selectedModelIndex = 0;
    });
  }

  void _activateMode(ChatComposerMode mode) {
    _suppressCommandSuggestions = true;
    widget.controller.value = const TextEditingValue(
      text: '/',
      selection: TextSelection.collapsed(offset: 1),
    );
    _updateCommandSuggestions();
    widget.onModeChanged?.call(mode);
  }

  void _selectCommand(_ComposerSlashCommand command) {
    switch (command) {
      case _ComposerSlashCommand.newConversation:
        widget.controller.clear();
        _updateCommandSuggestions();
        widget.onNewConversation?.call();
      case _ComposerSlashCommand.search:
        widget.controller.clear();
        _updateCommandSuggestions();
        widget.onOpenHistorySearch?.call(null);
      case _ComposerSlashCommand.summarize:
        widget.controller.clear();
        _updateCommandSuggestions();
        widget.onSummarizeConversation?.call(null);
      case _ComposerSlashCommand.rename:
      case _ComposerSlashCommand.reasoning:
        _prepareArgumentCommand(command);
      case _ComposerSlashCommand.model:
        _suppressCommandSuggestions = true;
        widget.controller.value = const TextEditingValue(
          text: '/',
          selection: TextSelection.collapsed(offset: 1),
        );
        setState(() {
          _showingModelPicker = true;
          _selectedModelIndex = 0;
        });
        widget.onRefreshSlashModels?.call();
      case _ComposerSlashCommand.compact:
        widget.controller.clear();
        _updateCommandSuggestions();
        widget.onCompactCommand?.call(null);
      case _ComposerSlashCommand.plan:
        _activateMode(ChatComposerMode.plan);
      case _ComposerSlashCommand.goal:
        _activateMode(ChatComposerMode.goal);
    }
  }

  void _prepareArgumentCommand(_ComposerSlashCommand command) {
    final value = '${command.name} ';
    widget.controller.value = TextEditingValue(
      text: value,
      selection: TextSelection.collapsed(offset: value.length),
    );
    _updateCommandSuggestions();
  }

  KeyEventResult _handleKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) return KeyEventResult.ignored;
    if (_showingModelPicker) {
      final models = _availableModels;
      if (event.logicalKey == LogicalKeyboardKey.escape) {
        widget.controller.clear();
        setState(() {
          _showingModelPicker = false;
          _suppressCommandSuggestions = false;
        });
        return KeyEventResult.handled;
      }
      if (models.isNotEmpty &&
          (event.logicalKey == LogicalKeyboardKey.arrowDown ||
              event.logicalKey == LogicalKeyboardKey.arrowUp)) {
        setState(() {
          final currentIndex = _selectedModelIndex % models.length;
          final direction = event.logicalKey == LogicalKeyboardKey.arrowDown
              ? 1
              : -1;
          _selectedModelIndex =
              (currentIndex + direction + models.length) % models.length;
        });
        return KeyEventResult.handled;
      }
      if (models.isNotEmpty &&
          widget.onModelSelected != null &&
          (event.logicalKey == LogicalKeyboardKey.enter ||
              event.logicalKey == LogicalKeyboardKey.numpadEnter)) {
        _chooseModel(models[_selectedModelIndex % models.length]);
        return KeyEventResult.handled;
      }
    }
    if (_matchingCommands.isNotEmpty) {
      if (event.logicalKey == LogicalKeyboardKey.arrowDown ||
          event.logicalKey == LogicalKeyboardKey.arrowUp) {
        setState(() {
          final direction = event.logicalKey == LogicalKeyboardKey.arrowDown
              ? 1
              : -1;
          _selectedCommandIndex =
              (_selectedCommandIndex + direction + _matchingCommands.length) %
              _matchingCommands.length;
        });
        return KeyEventResult.handled;
      }
      if (event.logicalKey == LogicalKeyboardKey.tab ||
          ((event.logicalKey == LogicalKeyboardKey.enter ||
                  event.logicalKey == LogicalKeyboardKey.numpadEnter) &&
              !HardwareKeyboard.instance.isShiftPressed)) {
        _selectCommand(_matchingCommands[_selectedCommandIndex]);
        return KeyEventResult.handled;
      }
    }
    if (widget.isSending && event.logicalKey == LogicalKeyboardKey.escape) {
      widget.onStopMessage?.call();
      return KeyEventResult.handled;
    }
    if ((event.logicalKey == LogicalKeyboardKey.enter ||
            event.logicalKey == LogicalKeyboardKey.numpadEnter) &&
        !HardwareKeyboard.instance.isShiftPressed) {
      if (widget.canSendMessage &&
          (widget.controller.text.trim().isNotEmpty ||
              widget.hasPendingAttachments)) {
        widget.onSendMessage();
      }
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  List<ChatGptModel> get _availableModels => widget.slashModels
      .where(
        (model) =>
            (model.isAvailable ||
                const {
                  'llama_cpp',
                  'vllm',
                  'exllama',
                }.contains(model.providerId)) &&
            !widget.hiddenModelKeys.contains(model.routeKey) &&
            !widget.hiddenModelKeys.contains('${model.providerId}:${model.id}'),
      )
      .toList(growable: false);

  void _chooseModel(ChatGptModel model) {
    widget.controller.clear();
    setState(() {
      _showingModelPicker = false;
      _suppressCommandSuggestions = false;
    });
    widget.onModelSelected?.call(model);
  }

  @override
  Widget build(BuildContext context) {
    final suggestionMenuMaxHeight = (MediaQuery.sizeOf(context).height * 0.4)
        .clamp(120.0, 280.0)
        .toDouble();
    final reducedMotion = MediaQuery.disableAnimationsOf(context);
    final menuCommandsKey = _showingModelPicker
        ? 'model-picker'
        : _matchingCommands.map((command) => command.name).join(',');
    return Focus(
      skipTraversal: true,
      onKeyEvent: _handleKeyEvent,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          AnimatedSwitcher(
            duration: reducedMotion
                ? Duration.zero
                : const Duration(milliseconds: 180),
            reverseDuration: reducedMotion
                ? Duration.zero
                : const Duration(milliseconds: 130),
            switchInCurve: Curves.easeOutCubic,
            switchOutCurve: Curves.easeOutCubic,
            transitionBuilder: (child, animation) {
              final eased = CurvedAnimation(
                parent: animation,
                curve: Curves.easeOutCubic,
              );
              return ClipRect(
                child: SizeTransition(
                  axis: Axis.vertical,
                  alignment: Alignment.topCenter,
                  sizeFactor: eased,
                  child: FadeTransition(
                    opacity: eased,
                    child: SlideTransition(
                      position: Tween<Offset>(
                        begin: const Offset(0, -0.06),
                        end: Offset.zero,
                      ).animate(eased),
                      child: child,
                    ),
                  ),
                ),
              );
            },
            child: !_showingModelPicker && _matchingCommands.isEmpty
                ? const SizedBox.shrink(key: ValueKey('empty-slash-menu'))
                : Column(
                    key: ValueKey<String>(menuCommandsKey),
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Padding(
                        padding: EdgeInsets.symmetric(
                          horizontal: widget.menuHorizontalPadding,
                        ),
                        child: _showingModelPicker
                            ? _SlashModelMenu(
                                models: _availableModels,
                                selectedIndex: _selectedModelIndex,
                                isLoading: widget.isLoadingSlashModels,
                                canSelect: widget.onModelSelected != null,
                                palette: widget.palette,
                                maxHeight: suggestionMenuMaxHeight,
                                providerNames: widget.providerNames,
                                onSelect: _chooseModel,
                              )
                            : _SlashCommandMenu(
                                commands: _matchingCommands,
                                selectedCommandIndex: _selectedCommandIndex,
                                palette: widget.palette,
                                maxHeight: suggestionMenuMaxHeight,
                                onSelect: _selectCommand,
                              ),
                      ),
                      const SizedBox(height: 8),
                    ],
                  ),
          ),
          widget.child,
        ],
      ),
    );
  }
}

class _SlashCommandMenu extends StatefulWidget {
  const _SlashCommandMenu({
    required this.commands,
    required this.selectedCommandIndex,
    required this.palette,
    required this.maxHeight,
    required this.onSelect,
  });

  final List<_ComposerSlashCommand> commands;
  final int selectedCommandIndex;
  final OpenChatPalette palette;
  final double maxHeight;
  final ValueChanged<_ComposerSlashCommand> onSelect;

  @override
  State<_SlashCommandMenu> createState() => _SlashCommandMenuState();
}

class _SlashCommandMenuState extends State<_SlashCommandMenu> {
  late List<GlobalKey> _itemKeys;

  @override
  void initState() {
    super.initState();
    _itemKeys = List<GlobalKey>.generate(
      widget.commands.length,
      (_) => GlobalKey(),
    );
  }

  @override
  void didUpdateWidget(covariant _SlashCommandMenu oldWidget) {
    super.didUpdateWidget(oldWidget);
    final commandsChanged = !_sameCommands(oldWidget.commands, widget.commands);
    if (commandsChanged) {
      _itemKeys = List<GlobalKey>.generate(
        widget.commands.length,
        (_) => GlobalKey(),
      );
    }
    if (commandsChanged ||
        oldWidget.selectedCommandIndex != widget.selectedCommandIndex) {
      _ensureSelectedItemVisible();
    }
  }

  bool _sameCommands(
    List<_ComposerSlashCommand> first,
    List<_ComposerSlashCommand> second,
  ) =>
      first.length == second.length &&
      first.indexed.every((entry) => entry.$2 == second[entry.$1]);

  void _ensureSelectedItemVisible() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || widget.commands.isEmpty) return;
      final index = widget.selectedCommandIndex
          .clamp(0, widget.commands.length - 1)
          .toInt();
      final target = _itemKeys[index].currentContext;
      if (target == null) return;
      Scrollable.ensureVisible(
        target,
        duration: MediaQuery.disableAnimationsOf(context)
            ? Duration.zero
            : const Duration(milliseconds: 120),
        curve: Curves.easeOutCubic,
        alignment: 0.5,
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: widget.palette.surface,
        border: Border.all(color: widget.palette.controlBorder),
        borderRadius: BorderRadius.circular(OpenChatRadii.menu),
      ),
      child: ConstrainedBox(
        constraints: BoxConstraints(maxHeight: widget.maxHeight),
        child: ListView.separated(
          shrinkWrap: true,
          padding: const EdgeInsets.all(5),
          physics: const ClampingScrollPhysics(),
          itemCount: widget.commands.length,
          separatorBuilder: (context, index) => const SizedBox(height: 2),
          itemBuilder: (context, index) => KeyedSubtree(
            key: _itemKeys[index],
            child: _SlashCommandSuggestion(
              command: widget.commands[index],
              selected: index == widget.selectedCommandIndex,
              palette: widget.palette,
              onTap: () => widget.onSelect(widget.commands[index]),
            ),
          ),
        ),
      ),
    );
  }
}

class _SlashModelMenu extends StatefulWidget {
  const _SlashModelMenu({
    required this.models,
    required this.selectedIndex,
    required this.isLoading,
    required this.canSelect,
    required this.palette,
    required this.maxHeight,
    required this.providerNames,
    required this.onSelect,
  });

  final List<ChatGptModel> models;
  final int selectedIndex;
  final bool isLoading;
  final bool canSelect;
  final OpenChatPalette palette;
  final double maxHeight;
  final Map<String, String> providerNames;
  final ValueChanged<ChatGptModel> onSelect;

  @override
  State<_SlashModelMenu> createState() => _SlashModelMenuState();
}

class _SlashModelMenuState extends State<_SlashModelMenu> {
  late Map<String, GlobalKey> _modelKeys;

  @override
  void initState() {
    super.initState();
    _modelKeys = _createModelKeys(widget.models);
  }

  @override
  void didUpdateWidget(covariant _SlashModelMenu oldWidget) {
    super.didUpdateWidget(oldWidget);
    final modelsChanged = !_sameModels(oldWidget.models, widget.models);
    if (modelsChanged) _modelKeys = _createModelKeys(widget.models);
    if (modelsChanged || oldWidget.selectedIndex != widget.selectedIndex) {
      _ensureSelectedModelVisible();
    }
  }

  Map<String, GlobalKey> _createModelKeys(List<ChatGptModel> models) => {
    for (final model in models) model.routeKey: GlobalKey(),
  };

  bool _sameModels(List<ChatGptModel> first, List<ChatGptModel> second) =>
      first.length == second.length &&
      first.indexed.every(
        (entry) => entry.$2.routeKey == second[entry.$1].routeKey,
      );

  void _ensureSelectedModelVisible() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || widget.models.isEmpty) return;
      final index = widget.selectedIndex
          .clamp(0, widget.models.length - 1)
          .toInt();
      final target = _modelKeys[widget.models[index].routeKey]?.currentContext;
      if (target == null) return;
      Scrollable.ensureVisible(
        target,
        duration: MediaQuery.disableAnimationsOf(context)
            ? Duration.zero
            : const Duration(milliseconds: 120),
        curve: Curves.easeOutCubic,
        alignment: 0.5,
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final providers = <String, List<ChatGptModel>>{};
    for (final model in widget.models) {
      providers.putIfAbsent(model.providerId, () => []).add(model);
    }
    final providerIds = providers.keys.toList()
      ..sort(
        (a, b) => (widget.providerNames[a] ?? _providerLabel(a, l10n))
            .compareTo(widget.providerNames[b] ?? _providerLabel(b, l10n)),
      );
    final items = <Widget>[];
    var modelIndex = 0;
    for (final providerId in providerIds) {
      items.add(
        Padding(
          padding: const EdgeInsets.fromLTRB(10, 8, 10, 4),
          child: Text(
            widget.providerNames[providerId] ??
                _providerLabel(providerId, l10n),
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: widget.palette.secondaryText,
              fontWeight: FontWeight.w700,
            ),
          ),
        ),
      );
      for (final model in providers[providerId]!) {
        final selected = modelIndex == widget.selectedIndex;
        final enabled = widget.canSelect;
        items.add(
          KeyedSubtree(
            key: _modelKeys[model.routeKey],
            child: Material(
              color: selected ? widget.palette.hover : Colors.transparent,
              borderRadius: BorderRadius.circular(OpenChatRadii.menu),
              child: InkWell(
                onTap: enabled ? () => widget.onSelect(model) : null,
                borderRadius: BorderRadius.circular(OpenChatRadii.menu),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 10,
                    vertical: 8,
                  ),
                  child: Row(
                    children: [
                      ProviderIcon(
                        providerId: model.providerId,
                        color: enabled
                            ? widget.palette.secondaryIcon
                            : widget.palette.disabledIcon,
                        size: 16,
                      ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              model.displayName,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: Theme.of(context).textTheme.bodySmall
                                  ?.copyWith(
                                    color: enabled
                                        ? widget.palette.text
                                        : widget.palette.secondaryText,
                                  ),
                            ),
                            if (model.sourceLabel case final sourceLabel?)
                              Text(
                                sourceLabel,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: Theme.of(context).textTheme.labelSmall
                                    ?.copyWith(
                                      color: widget.palette.secondaryText,
                                    ),
                              ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        );
        modelIndex++;
      }
    }
    return DecoratedBox(
      decoration: BoxDecoration(
        color: widget.palette.surface,
        border: Border.all(color: widget.palette.controlBorder),
        borderRadius: BorderRadius.circular(OpenChatRadii.menu),
      ),
      child: ConstrainedBox(
        constraints: BoxConstraints(maxHeight: widget.maxHeight),
        child: widget.isLoading && widget.models.isEmpty
            ? Padding(
                padding: const EdgeInsets.all(18),
                child: Center(
                  child: SizedBox.square(
                    dimension: 18,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: widget.palette.accent,
                    ),
                  ),
                ),
              )
            : widget.models.isEmpty
            ? Padding(
                padding: const EdgeInsets.all(12),
                child: Text(
                  l10n.noModelsAvailable,
                  style: Theme.of(context).textTheme.bodySmall
                      ?.copyWith(color: widget.palette.secondaryText),
                ),
              )
            : ListView(
                shrinkWrap: true,
                padding: const EdgeInsets.all(5),
                physics: const ClampingScrollPhysics(),
                children: items,
              ),
      ),
    );
  }
}

String _providerLabel(String providerId, AppLocalizations l10n) =>
    switch (providerId) {
      'chatgpt' => l10n.chatGptProvider,
      'chatgpt_api' => '${l10n.chatGptProvider} API',
      'opencode' => l10n.openCodeProvider,
      'gemini' => l10n.geminiProvider,
      'groq' => l10n.groqProvider,
      'cerebras' => l10n.cerebrasProvider,
      'openrouter' => l10n.openRouterProvider,
      'mistral' => l10n.mistralProvider,
      'llama_cpp' => 'llama.cpp',
      'vllm' => 'vLLM',
      'exllama' => 'ExLlamaV3',
      _ => providerId,
    };

class _SlashCommandSuggestion extends StatelessWidget {
  const _SlashCommandSuggestion({
    required this.command,
    required this.selected,
    required this.palette,
    required this.onTap,
  });

  final _ComposerSlashCommand command;
  final bool selected;
  final OpenChatPalette palette;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final theme = Theme.of(context);
    final label = switch (command) {
      _ComposerSlashCommand.newConversation => l10n.slashCommandNewLabel,
      _ComposerSlashCommand.search => l10n.slashCommandSearchLabel,
      _ComposerSlashCommand.summarize => l10n.slashCommandSummarizeLabel,
      _ComposerSlashCommand.rename => l10n.slashCommandRenameLabel,
      _ComposerSlashCommand.model => l10n.slashCommandModelLabel,
      _ComposerSlashCommand.compact => l10n.slashCommandCompactLabel,
      _ComposerSlashCommand.reasoning => l10n.slashCommandReasoningLabel,
      _ComposerSlashCommand.plan => l10n.slashCommandPlanLabel,
      _ComposerSlashCommand.goal => l10n.slashCommandGoalLabel,
    };
    final description = switch (command) {
      _ComposerSlashCommand.newConversation => l10n.newConversation,
      _ComposerSlashCommand.search => l10n.searchChatsHint,
      _ComposerSlashCommand.summarize => l10n.slashCommandSummarizeDescription,
      _ComposerSlashCommand.rename => l10n.slashCommandRenameDescription,
      _ComposerSlashCommand.model => l10n.slashCommandModelDescription,
      _ComposerSlashCommand.compact => l10n.slashCommandCompactDescription,
      _ComposerSlashCommand.reasoning => l10n.slashCommandReasoningDescription,
      _ComposerSlashCommand.plan => l10n.toolPermissionPlanDescription,
      _ComposerSlashCommand.goal => l10n.goalCommandDescription,
    };
    final icon = switch (command) {
      _ComposerSlashCommand.newConversation => LucideIcons.squarePen,
      _ComposerSlashCommand.search => LucideIcons.search,
      _ComposerSlashCommand.summarize => LucideIcons.textQuote,
      _ComposerSlashCommand.rename => LucideIcons.pencil,
      _ComposerSlashCommand.model => LucideIcons.bot,
      _ComposerSlashCommand.compact => LucideIcons.archive,
      _ComposerSlashCommand.reasoning => LucideIcons.brain,
      _ComposerSlashCommand.plan => LucideIcons.listTodo,
      _ComposerSlashCommand.goal => LucideIcons.flag,
    };
    final radius = BorderRadius.circular(OpenChatRadii.menu);
    return Semantics(
      button: true,
      selected: selected,
      label: '$label. $description',
      child: Material(
        color: selected ? palette.hover : Colors.transparent,
        borderRadius: radius,
        child: InkWell(
          onTap: onTap,
          borderRadius: radius,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Icon(icon, size: 16, color: palette.secondaryIcon),
                const SizedBox(width: 10),
                SizedBox(
                  width: 112,
                  child: Text(
                    label,
                    style: theme.textTheme.labelMedium?.copyWith(
                      color: palette.text,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
                Expanded(
                  child: Text(
                    description,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: palette.secondaryText,
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

class _ComposerActions extends StatelessWidget {
  const _ComposerActions({
    required this.controller,
    required this.onSendMessage,
    required this.canSendMessage,
    required this.showReasoningSelector,
    this.toolPermissionMode = ToolPermissionMode.requireApproval,
    this.onToolPermissionModeChanged,
    required this.mode,
    required this.onModeChanged,
    required this.isLoadingModels,
    required this.models,
    required this.favoriteModels,
    required this.providerId,
    required this.isChatGptConnected,
    required this.chatGptFastModeEnabled,
    required this.chatGptFastModeAvailable,
    required this.chatGptFastModeLoading,
    required this.chatGptFastModeSaving,
    required this.onChatGptFastModeChanged,
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
    required this.modelPricingRepository,
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
  final ChatComposerMode mode;
  final ValueChanged<ChatComposerMode>? onModeChanged;
  final bool isLoadingModels;
  final List<ChatGptModel> models;
  final List<FavoriteModel> favoriteModels;
  final String providerId;
  final bool isChatGptConnected;
  final bool chatGptFastModeEnabled;
  final bool chatGptFastModeAvailable;
  final bool chatGptFastModeLoading;
  final bool chatGptFastModeSaving;
  final ValueChanged<bool>? onChatGptFastModeChanged;
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
  final ModelPricingRepository? modelPricingRepository;
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
    final reducedMotion = MediaQuery.disableAnimationsOf(context);
    final semantic = OpenChatSemanticColors.of(context);
    final primary = semantic.primary;
    final primaryForeground = semantic.primaryForeground;
    final selectedModelLabel = modelLabel?.trim();
    final modelSelector = ModelSelector(
      label: selectedModelLabel == null || selectedModelLabel.isEmpty
          ? l10n.modelSelection
          : selectedModelLabel,
      palette: palette,
      compact: touchTargets,
      borderless: true,
      width: narrow ? 144 : 152,
      models: models,
      pricingRepository: modelPricingRepository,
      favoriteModels: favoriteModels,
      selectedModelId: selectedModelId,
      selectedModelRouteKey: selectedModelRouteKey,
      providerId: providerId,
      showReasoningSelector: showReasoningSelector,
      reasoningLevel: reasoningLevel,
      reasoningOptions: reasoningOptions,
      onReasoningSelected: onReasoningSelected,
      isChatGptConnected: isChatGptConnected,
      chatGptFastModeEnabled: chatGptFastModeEnabled,
      chatGptFastModeAvailable: chatGptFastModeAvailable,
      chatGptFastModeLoading: chatGptFastModeLoading,
      chatGptFastModeSaving: chatGptFastModeSaving,
      onChatGptFastModeChanged: onChatGptFastModeChanged,
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
    final toolPermissionSelector = _ToolPermissionSelector(
      mode: toolPermissionMode,
      palette: palette,
      width: narrow ? 168 : 176,
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
        onTap: attachmentsEnabled ? onAddAttachments : null,
        child: ExcludeSemantics(
          child: SizedBox.square(
            dimension: attachmentSize,
            child: Focus(
              skipTraversal: true,
              child: Builder(
                builder: (focusContext) {
                  final focused = Focus.of(focusContext).hasFocus;
                  return shad.ShadIconButton.outline(
                    icon: Icon(
                      LucideIcons.paperclip,
                      size: 18,
                      color: attachmentsEnabled
                          ? palette.secondaryIcon
                          : palette.disabledIcon,
                    ),
                    onPressed: attachmentsEnabled ? onAddAttachments : null,
                    enabled: attachmentsEnabled,
                    width: attachmentSize,
                    height: attachmentSize,
                    padding: EdgeInsets.zero,
                    backgroundColor: focused
                        ? palette.hover
                        : Colors.transparent,
                    foregroundColor: attachmentsEnabled
                        ? palette.secondaryIcon
                        : palette.disabledIcon,
                    decoration: const shad.ShadDecoration(
                      shape: BoxShape.circle,
                      border: shad.ShadBorder.none,
                      focusedBorder: shad.ShadBorder.none,
                    ),
                  );
                },
              ),
            ),
          ),
        ),
      ),
    );
    final sendButton = ValueListenableBuilder<TextEditingValue>(
      valueListenable: controller,
      builder: (context, value, _) {
        final canAttemptSend =
            canSendMessage &&
            (value.text.trim().isNotEmpty || pendingAttachments.isNotEmpty);
        final actionEnabled = isSending
            ? onStopMessage != null
            : canAttemptSend;
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
            onTap: actionEnabled ? onPressed : null,
            child: ExcludeSemantics(
              child: SizedBox.square(
                dimension: sendSize,
                child: shad.ShadIconButton.raw(
                  variant: actionEnabled
                      ? shad.ShadButtonVariant.primary
                      : shad.ShadButtonVariant.secondary,
                  icon: isSending
                      ? const Icon(LucideIcons.square)
                      : const Icon(LucideIcons.arrowUp, size: 18),
                  iconSize: 18,
                  onPressed: onPressed,
                  enabled: actionEnabled,
                  width: sendSize,
                  height: sendSize,
                  padding: EdgeInsets.zero,
                  backgroundColor: actionEnabled ? primary : palette.selected,
                  foregroundColor: actionEnabled
                      ? primaryForeground
                      : palette.disabledForeground,
                  decoration: const shad.ShadDecoration(shape: BoxShape.circle),
                ),
              ),
            ),
          ),
        );
      },
    );
    final selectors = Wrap(
      spacing: 8,
      runSpacing: 8,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        modelSelector,
        toolPermissionSelector,
        AnimatedSwitcher(
          duration: reducedMotion
              ? Duration.zero
              : const Duration(milliseconds: 190),
          reverseDuration: reducedMotion
              ? Duration.zero
              : const Duration(milliseconds: 130),
          switchInCurve: Curves.easeOutCubic,
          switchOutCurve: Curves.easeOutCubic,
          transitionBuilder: (child, animation) {
            final eased = CurvedAnimation(
              parent: animation,
              curve: Curves.easeOutCubic,
            );
            return ClipRect(
              child: SizeTransition(
                axis: Axis.horizontal,
                alignment: AlignmentDirectional.centerStart,
                sizeFactor: eased,
                child: FadeTransition(
                  opacity: eased,
                  child: SlideTransition(
                    position: Tween<Offset>(
                      begin: const Offset(0.08, 0),
                      end: Offset.zero,
                    ).animate(eased),
                    child: child,
                  ),
                ),
              ),
            );
          },
          child: mode == ChatComposerMode.none
              ? const SizedBox.shrink(key: ValueKey(ChatComposerMode.none))
              : _ComposerModeControl(
                  key: ValueKey<ChatComposerMode>(mode),
                  mode: mode,
                  palette: palette,
                  touchTargets: touchTargets,
                  onDeactivate: () =>
                      onModeChanged?.call(ChatComposerMode.none),
                ),
        ),
        if (selectedModelId != null)
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
            toolPermissionMode: mode == ChatComposerMode.plan
                ? ToolPermissionMode.plan
                : toolPermissionMode,
            pendingAttachments: pendingAttachments,
          ),
      ],
    );

    return Row(
      crossAxisAlignment: CrossAxisAlignment.end,
      children: [
        Expanded(child: selectors),
        const SizedBox(width: 8),
        attachmentButton,
        const SizedBox(width: 4),
        sendButton,
      ],
    );
  }
}

class _ComposerModeControl extends StatelessWidget {
  const _ComposerModeControl({
    required this.mode,
    required this.palette,
    required this.touchTargets,
    required this.onDeactivate,
    super.key,
  });

  final ChatComposerMode mode;
  final OpenChatPalette palette;
  final bool touchTargets;
  final VoidCallback onDeactivate;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final modeColor = OpenChatSemanticColors.of(context).primary;
    final buttonSize = touchTargets ? 44.0 : 36.0;
    final label = switch (mode) {
      ChatComposerMode.plan => l10n.toolPermissionPlan,
      ChatComposerMode.goal => l10n.composerGoalModeLabel,
      ChatComposerMode.none => '',
    };
    final offLabel = switch (mode) {
      ChatComposerMode.plan => l10n.toolPermissionPlanOff,
      ChatComposerMode.goal => l10n.composerGoalModeOff,
      ChatComposerMode.none => '',
    };
    final icon = switch (mode) {
      ChatComposerMode.plan => LucideIcons.listTodo,
      ChatComposerMode.goal => LucideIcons.flag,
      ChatComposerMode.none => LucideIcons.x,
    };
    return Tooltip(
      message: offLabel,
      child: Semantics(
        button: true,
        selected: true,
        label: '$label, $offLabel',
        onTap: onDeactivate,
        child: ExcludeSemantics(
          child: shad.ShadButton.ghost(
            height: buttonSize,
            expands: false,
            padding: const EdgeInsetsDirectional.only(start: 10, end: 8),
            enabled: true,
            onPressed: onDeactivate,
            backgroundColor: palette.selected,
            hoverBackgroundColor: palette.hover,
            foregroundColor: palette.text,
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(icon, size: 16, color: modeColor),
                const SizedBox(width: 7),
                Text(
                  label,
                  style: Theme.of(context).textTheme.labelLarge?.copyWith(
                    color: palette.text,
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    height: 18 / 13,
                  ),
                ),
                const SizedBox(width: 8),
                Icon(LucideIcons.x, size: 14, color: palette.secondaryIcon),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _ToolPermissionSelector extends StatelessWidget {
  const _ToolPermissionSelector({
    required this.mode,
    required this.palette,
    required this.width,
    required this.hasSelectedModel,
    required this.supportsToolCalls,
    required this.onSelected,
  });

  final ToolPermissionMode mode;
  final OpenChatPalette palette;
  final double width;
  final bool hasSelectedModel;
  final bool? supportsToolCalls;
  final ValueChanged<ToolPermissionMode>? onSelected;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final warning = OpenChatSemanticColors.of(context).warning;
    final label = switch (mode) {
      ToolPermissionMode.plan => l10n.toolPermissionPlan,
      ToolPermissionMode.requireApproval => l10n.toolPermissionRequireApproval,
      ToolPermissionMode.approveSafeOperations =>
        l10n.toolPermissionApproveSafeOperations,
      ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccess,
    };
    final description = switch (mode) {
      ToolPermissionMode.plan => l10n.toolPermissionPlanDescription,
      ToolPermissionMode.requireApproval =>
        l10n.toolPermissionRequireApprovalDescription,
      ToolPermissionMode.approveSafeOperations =>
        l10n.toolPermissionApproveSafeOperationsDescription,
      ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccessDescription,
    };
    final modeIcon = switch (mode) {
      ToolPermissionMode.plan => LucideIcons.listTodo,
      ToolPermissionMode.requireApproval => LucideIcons.hand,
      ToolPermissionMode.approveSafeOperations => LucideIcons.shieldCheck,
      ToolPermissionMode.fullAccess => LucideIcons.shieldAlert,
    };
    final modeColor = mode == ToolPermissionMode.fullAccess
        ? warning
        : palette.secondaryIcon;
    final options = [
      for (final option in <ToolPermissionMode>[
        ToolPermissionMode.requireApproval,
        ToolPermissionMode.approveSafeOperations,
        ToolPermissionMode.fullAccess,
      ])
        OpenChatSelectOption<ToolPermissionMode>(
          value: option,
          label: switch (option) {
            ToolPermissionMode.plan => l10n.toolPermissionPlan,
            ToolPermissionMode.requireApproval =>
              l10n.toolPermissionRequireApproval,
            ToolPermissionMode.approveSafeOperations =>
              l10n.toolPermissionApproveSafeOperations,
            ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccess,
          },
          description: switch (option) {
            ToolPermissionMode.plan => l10n.toolPermissionPlanDescription,
            ToolPermissionMode.requireApproval =>
              l10n.toolPermissionRequireApprovalDescription,
            ToolPermissionMode.approveSafeOperations =>
              l10n.toolPermissionApproveSafeOperationsDescription,
            ToolPermissionMode.fullAccess =>
              l10n.toolPermissionFullAccessDescription,
          },
          icon: switch (option) {
            ToolPermissionMode.plan => LucideIcons.listTodo,
            ToolPermissionMode.requireApproval => LucideIcons.hand,
            ToolPermissionMode.approveSafeOperations => LucideIcons.shieldCheck,
            ToolPermissionMode.fullAccess => LucideIcons.shieldAlert,
          },
          iconColor: option == ToolPermissionMode.fullAccess
              ? warning
              : palette.secondaryText,
          descriptionColor: option == ToolPermissionMode.fullAccess
              ? warning
              : palette.secondaryText,
          selectedColor: option == ToolPermissionMode.fullAccess
              ? warning
              : null,
          textStyle: option == ToolPermissionMode.fullAccess
              ? TextStyle(color: warning, fontSize: 13)
              : null,
        ),
    ];
    final selector = SizedBox(
      width: width,
      height: composerControlHeight(context),
      child: OpenChatSelect<ToolPermissionMode>(
        options: options,
        value: mode,
        onChanged: onSelected,
        palette: palette,
        width: width,
        menuWidth: 440,
        height: composerControlHeight(context),
        horizontalPadding: 10,
        borderless: true,
        selectedContent: Row(
          children: [
            Icon(modeIcon, size: 16, color: modeColor),
            const SizedBox(width: 7),
            Flexible(
              child: Text(
                label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.labelLarge?.copyWith(
                  color: mode == ToolPermissionMode.fullAccess
                      ? warning
                      : palette.text,
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                  height: 18 / 13,
                ),
              ),
            ),
          ],
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
      message: capabilityNotice == null
          ? description
          : '$description\n$capabilityNotice',
      child: _HoverSurface(palette: palette, child: selector),
    );
  }
}

class _HoverSurface extends StatefulWidget {
  const _HoverSurface({required this.palette, required this.child});

  final OpenChatPalette palette;
  final Widget child;

  @override
  State<_HoverSurface> createState() => _HoverSurfaceState();
}

class _HoverSurfaceState extends State<_HoverSurface> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: AnimatedContainer(
        duration: MediaQuery.disableAnimationsOf(context)
            ? Duration.zero
            : const Duration(milliseconds: 120),
        decoration: BoxDecoration(
          color: _hovered ? widget.palette.hover : Colors.transparent,
          borderRadius: BorderRadius.circular(OpenChatRadii.button),
        ),
        child: widget.child,
      ),
    );
  }
}
