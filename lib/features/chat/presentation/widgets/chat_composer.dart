import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../../../app/zihora_theme.dart';
import '../../../../l10n/zihora_localizations.dart';
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
      favoriteModels: favoriteModels,
      selectedModelId: selectedModelId,
      providerId: providerId,
      onProviderSelected: onProviderSelected,
      isLoadingModels: isLoadingModels,
      emptyModelsLabel:
          modelsEmptyLabel ?? context.zihoraL10n.noModelsAvailable,
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

class _ModelSelector extends StatefulWidget {
  const _ModelSelector({
    required this.label,
    required this.iconRoot,
    required this.palette,
    required this.compact,
    required this.models,
    required this.favoriteModels,
    required this.selectedModelId,
    required this.providerId,
    required this.onProviderSelected,
    required this.isLoadingModels,
    required this.emptyModelsLabel,
    required this.onSelected,
    required this.onFavoriteChanged,
    required this.onFavoriteSelected,
  });

  final String label;
  final String iconRoot;
  final ZihoraPalette palette;
  final bool compact;
  final List<ChatGptModel> models;
  final List<FavoriteModel> favoriteModels;
  final String? selectedModelId;
  final String providerId;
  final ValueChanged<String>? onProviderSelected;
  final bool isLoadingModels;
  final String emptyModelsLabel;
  final ValueChanged<String>? onSelected;
  final void Function(
    String providerId,
    String modelId,
    String displayName,
    bool isFavorite,
  )?
  onFavoriteChanged;
  final ValueChanged<FavoriteModel>? onFavoriteSelected;

  @override
  State<_ModelSelector> createState() => _ModelSelectorState();
}

class _ModelSelectorState extends State<_ModelSelector> {
  static const double _menuHeight = 420;

  final _menuController = MenuController();
  final _searchController = TextEditingController();
  bool _showFavorites = false;
  String _searchQuery = '';

  @override
  void didUpdateWidget(covariant _ModelSelector oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!_showFavorites && oldWidget.providerId != widget.providerId) {
      _clearSearch();
    }
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  void _clearSearch() {
    _searchController.clear();
    _searchQuery = '';
  }

  void _selectFavorites() {
    if (_showFavorites) return;
    setState(() {
      _showFavorites = true;
      _clearSearch();
    });
  }

  void _selectProvider(String providerId) {
    if (widget.onProviderSelected == null) return;
    final scopeChanged = _showFavorites || widget.providerId != providerId;
    setState(() {
      _showFavorites = false;
      if (scopeChanged) _clearSearch();
    });
    widget.onProviderSelected!(providerId);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.zihoraL10n;
    final availableModels = widget.models
        .where((model) => model.isAvailable)
        .toList(growable: false);
    final favoriteModels = widget.onProviderSelected == null
        ? widget.favoriteModels
              .where((favorite) => favorite.providerId == widget.providerId)
              .toList(growable: false)
        : widget.favoriteModels;
    final normalizedQuery = _searchQuery.trim().toLowerCase();
    bool matchesQuery(Iterable<String> values) =>
        normalizedQuery.isEmpty ||
        values.any((value) => value.toLowerCase().contains(normalizedQuery));
    final visibleFavorites = favoriteModels
        .where(
          (favorite) => matchesQuery([
            favorite.displayName,
            favorite.modelId,
            favorite.providerId == 'chatgpt'
                ? l10n.chatGptProvider
                : l10n.openCodeProvider,
          ]),
        )
        .toList(growable: false);
    final visibleModels = availableModels
        .where(
          (model) => matchesQuery([
            model.displayName,
            model.id,
            if (model.description == 'paid') l10n.openCodePaidModel,
            if (model.description == 'free') l10n.openCodeFreeModel,
          ]),
        )
        .toList(growable: false);
    final shownCount = _showFavorites
        ? visibleFavorites.length
        : visibleModels.length;
    final screenWidth = MediaQuery.sizeOf(context).width;
    final menuWidth = math.max(0.0, math.min(520.0, screenWidth - 32));
    final providerWidth = math.min(
      menuWidth * 0.36,
      menuWidth < 420 ? 112.0 : 152.0,
    );

    return MenuAnchor(
      controller: _menuController,
      crossAxisUnconstrained: true,
      alignmentOffset: const Offset(0, 8),
      reservedPadding: const EdgeInsets.all(12),
      style: MenuStyle(
        backgroundColor: WidgetStatePropertyAll(widget.palette.surface),
        elevation: const WidgetStatePropertyAll(8),
        padding: const WidgetStatePropertyAll(EdgeInsets.zero),
        side: WidgetStatePropertyAll(BorderSide(color: widget.palette.border)),
        shape: WidgetStatePropertyAll(
          RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
        maximumSize: WidgetStatePropertyAll(Size(menuWidth, _menuHeight)),
      ),
      menuChildren: [
        SizedBox(
          width: menuWidth,
          height: _menuHeight,
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Container(
                width: providerWidth,
                color: widget.palette.navigation,
                padding: const EdgeInsets.all(8),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _ModelProviderTab(
                      providerId: 'favorites',
                      label: l10n.favoriteModels,
                      selected: _showFavorites,
                      onPressed: _selectFavorites,
                      palette: widget.palette,
                    ),
                    const SizedBox(height: 4),
                    _ModelProviderTab(
                      providerId: 'chatgpt',
                      label: l10n.chatGptProvider,
                      selected:
                          !_showFavorites && widget.providerId == 'chatgpt',
                      onPressed: widget.onProviderSelected == null
                          ? null
                          : () => _selectProvider('chatgpt'),
                      palette: widget.palette,
                    ),
                    const SizedBox(height: 4),
                    _ModelProviderTab(
                      providerId: 'opencode',
                      label: l10n.openCodeProvider,
                      selected:
                          !_showFavorites && widget.providerId == 'opencode',
                      onPressed: widget.onProviderSelected == null
                          ? null
                          : () => _selectProvider('opencode'),
                      palette: widget.palette,
                    ),
                  ],
                ),
              ),
              VerticalDivider(
                width: 1,
                thickness: 1,
                color: widget.palette.border,
              ),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(12, 12, 12, 8),
                  child: Column(
                    children: [
                      SizedBox(
                        height: 40,
                        child: TextField(
                          controller: _searchController,
                          onChanged: (value) =>
                              setState(() => _searchQuery = value),
                          textInputAction: TextInputAction.search,
                          style: TextStyle(
                            color: widget.palette.text,
                            fontSize: 13,
                          ),
                          decoration: InputDecoration(
                            hintText: l10n.modelSearchHint,
                            hintStyle: TextStyle(
                              color: widget.palette.secondaryText,
                              fontSize: 13,
                            ),
                            prefixIcon: Icon(
                              Icons.search,
                              size: 18,
                              color: widget.palette.secondaryIcon,
                            ),
                            prefixIconConstraints: const BoxConstraints(
                              minWidth: 36,
                            ),
                            isDense: true,
                            contentPadding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 9,
                            ),
                            filled: true,
                            fillColor: widget.palette.navigation,
                            enabledBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(8),
                              borderSide: BorderSide(
                                color: widget.palette.border,
                              ),
                            ),
                            focusedBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(8),
                              borderSide: BorderSide(
                                color: widget.palette.secondaryIcon,
                              ),
                            ),
                          ),
                        ),
                      ),
                      const SizedBox(height: 8),
                      Expanded(
                        child: shownCount == 0
                            ? Center(
                                child:
                                    !_showFavorites &&
                                        widget.isLoadingModels &&
                                        availableModels.isEmpty
                                    ? const SizedBox.square(
                                        dimension: 20,
                                        child: CircularProgressIndicator(
                                          strokeWidth: 2,
                                        ),
                                      )
                                    : Padding(
                                        padding: const EdgeInsets.all(16),
                                        child: Text(
                                          _showFavorites
                                              ? favoriteModels.isNotEmpty &&
                                                        normalizedQuery
                                                            .isNotEmpty
                                                    ? l10n.modelSearchNoResults
                                                    : l10n.favoriteModelsEmpty
                                              : availableModels.isNotEmpty &&
                                                    normalizedQuery.isNotEmpty
                                              ? l10n.modelSearchNoResults
                                              : widget.emptyModelsLabel,
                                          textAlign: TextAlign.center,
                                          style: TextStyle(
                                            color: widget.palette.secondaryText,
                                            fontSize: 13,
                                          ),
                                        ),
                                      ),
                              )
                            : ListView.builder(
                                padding: const EdgeInsets.symmetric(
                                  vertical: 4,
                                ),
                                itemCount: shownCount,
                                itemExtent: 44,
                                itemBuilder: (context, index) {
                                  if (_showFavorites) {
                                    final favorite = visibleFavorites[index];
                                    final providerLabel =
                                        favorite.providerId == 'chatgpt'
                                        ? l10n.chatGptProvider
                                        : l10n.openCodeProvider;
                                    return _ModelOption(
                                      key: ValueKey<String>(
                                        '${favorite.providerId}:${favorite.modelId}',
                                      ),
                                      title:
                                          '${favorite.displayName} · $providerLabel',
                                      selected:
                                          favorite.providerId ==
                                              widget.providerId &&
                                          favorite.modelId ==
                                              widget.selectedModelId,
                                      isFavorite: true,
                                      palette: widget.palette,
                                      addFavoriteLabel: l10n.addModelFavorite,
                                      removeFavoriteLabel:
                                          l10n.removeModelFavorite,
                                      onSelected:
                                          widget.onFavoriteSelected == null
                                          ? null
                                          : () {
                                              widget.onFavoriteSelected!(
                                                favorite,
                                              );
                                              _menuController.close();
                                            },
                                      onToggleFavorite:
                                          widget.onFavoriteChanged == null
                                          ? null
                                          : () => widget.onFavoriteChanged!(
                                              favorite.providerId,
                                              favorite.modelId,
                                              favorite.displayName,
                                              false,
                                            ),
                                    );
                                  }

                                  final model = visibleModels[index];
                                  final isFavorite = widget.favoriteModels.any(
                                    (favorite) =>
                                        favorite.providerId ==
                                            widget.providerId &&
                                        favorite.modelId == model.id,
                                  );
                                  final title = model.description == 'paid'
                                      ? '${model.displayName} · ${l10n.openCodePaidModel}'
                                      : model.description == 'free'
                                      ? '${model.displayName} · ${l10n.openCodeFreeModel}'
                                      : model.displayName;
                                  return _ModelOption(
                                    key: ValueKey<String>(model.id),
                                    title: title,
                                    selected:
                                        model.id == widget.selectedModelId,
                                    isFavorite: isFavorite,
                                    palette: widget.palette,
                                    addFavoriteLabel: l10n.addModelFavorite,
                                    removeFavoriteLabel:
                                        l10n.removeModelFavorite,
                                    onSelected: widget.onSelected == null
                                        ? null
                                        : () {
                                            widget.onSelected!(model.id);
                                            _menuController.close();
                                          },
                                    onToggleFavorite:
                                        widget.onFavoriteChanged == null
                                        ? null
                                        : () => widget.onFavoriteChanged!(
                                            widget.providerId,
                                            model.id,
                                            model.displayName,
                                            !isFavorite,
                                          ),
                                  );
                                },
                              ),
                      ),
                    ],
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
          onPressed:
              widget.onSelected == null && widget.onProviderSelected == null
              ? null
              : () =>
                    controller.isOpen ? controller.close() : controller.open(),
          style: _controlStyle(
            widget.palette,
            width: 132,
            compact: widget.compact,
            leftPadding: 12,
            rightPadding: 12,
          ),
          child: Row(
            children: [
              SizedBox(
                width: 16,
                height: 16,
                child: Center(
                  child: _providerIcon(
                    widget.providerId,
                    widget.palette.secondaryIcon,
                    16,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  widget.label,
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
                    '${widget.iconRoot}/chevron.svg',
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

class _ModelOption extends StatefulWidget {
  const _ModelOption({
    required this.title,
    required this.selected,
    required this.isFavorite,
    required this.palette,
    required this.addFavoriteLabel,
    required this.removeFavoriteLabel,
    required this.onSelected,
    required this.onToggleFavorite,
    super.key,
  });

  final String title;
  final bool selected;
  final bool isFavorite;
  final ZihoraPalette palette;
  final String addFavoriteLabel;
  final String removeFavoriteLabel;
  final VoidCallback? onSelected;
  final VoidCallback? onToggleFavorite;

  @override
  State<_ModelOption> createState() => _ModelOptionState();
}

class _ModelOptionState extends State<_ModelOption> {
  bool _hovered = false;
  bool _rowFocused = false;
  bool _favoriteFocused = false;

  @override
  Widget build(BuildContext context) {
    final showFavorite =
        _hovered || _rowFocused || _favoriteFocused || widget.isFavorite;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: widget.selected || _hovered
              ? widget.palette.selected
              : Colors.transparent,
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          children: [
            Expanded(
              child: InkWell(
                onTap: widget.onSelected,
                onFocusChange: (focused) => setState(() {
                  _rowFocused = focused;
                }),
                borderRadius: BorderRadius.circular(8),
                child: SizedBox(
                  height: 44,
                  child: Padding(
                    padding: const EdgeInsets.only(left: 12, right: 4),
                    child: Row(
                      children: [
                        SizedBox(
                          width: 20,
                          child: widget.selected
                              ? Icon(
                                  Icons.check_rounded,
                                  size: 18,
                                  color: widget.palette.accent,
                                )
                              : null,
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            widget.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: widget.palette.text,
                              fontSize: 13,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
            Focus(
              onFocusChange: (focused) => setState(() {
                _favoriteFocused = focused;
              }),
              child: AnimatedOpacity(
                opacity: showFavorite ? 1 : 0,
                duration: const Duration(milliseconds: 100),
                child: SizedBox(
                  width: 40,
                  child: IconButton(
                    tooltip: widget.isFavorite
                        ? widget.removeFavoriteLabel
                        : widget.addFavoriteLabel,
                    onPressed: showFavorite ? widget.onToggleFavorite : null,
                    visualDensity: VisualDensity.compact,
                    padding: EdgeInsets.zero,
                    iconSize: 18,
                    color: widget.isFavorite
                        ? widget.palette.accent
                        : widget.palette.secondaryIcon,
                    icon: Icon(
                      widget.isFavorite
                          ? Icons.star_rounded
                          : Icons.star_border_rounded,
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ModelProviderTab extends StatelessWidget {
  const _ModelProviderTab({
    required this.providerId,
    required this.label,
    required this.selected,
    required this.onPressed,
    required this.palette,
  });

  final String providerId;
  final String label;
  final bool selected;
  final VoidCallback? onPressed;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      selected: selected,
      child: TextButton(
        onPressed: onPressed,
        style: TextButton.styleFrom(
          alignment: Alignment.centerLeft,
          backgroundColor: selected ? palette.selected : Colors.transparent,
          foregroundColor: selected ? palette.text : palette.secondaryText,
          disabledForegroundColor: selected
              ? palette.text
              : palette.secondaryText,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          minimumSize: const Size(0, 40),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
        ),
        child: Row(
          children: [
            SizedBox(
              width: 18,
              height: 18,
              child: Center(
                child: _providerIcon(
                  providerId,
                  selected ? palette.secondaryIcon : palette.secondaryText,
                  18,
                ),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

Widget _providerIcon(String providerId, Color color, double size) {
  switch (providerId) {
    case 'chatgpt':
      return SvgPicture.asset(
        'assets/icons/chatgpt.svg',
        width: size,
        height: size,
        colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
        excludeFromSemantics: true,
      );
    case 'opencode':
      return SvgPicture.asset(
        'assets/icons/opencode.svg',
        width: size,
        height: size,
        colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
        excludeFromSemantics: true,
      );
    case 'favorites':
      return Icon(Icons.star_outline_rounded, size: size, color: color);
    default:
      return const SizedBox.shrink();
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
  final ZihoraPalette palette;
  final bool compact;
  final ValueChanged<ToolPermissionMode>? onSelected;

  @override
  Widget build(BuildContext context) {
    final l10n = context.zihoraL10n;
    final label = switch (mode) {
      ToolPermissionMode.requireApproval => l10n.toolPermissionRequireApproval,
      ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccess,
    };
    final description = switch (mode) {
      ToolPermissionMode.requireApproval =>
        l10n.toolPermissionRequireApprovalDescription,
      ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccessDescription,
    };

    return Tooltip(
      message: '$label\n$description',
      child: MenuAnchor(
        menuChildren: [
          for (final option in ToolPermissionMode.values)
            MenuItemButton(
              onPressed: onSelected == null ? null : () => onSelected!(option),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(switch (option) {
                    ToolPermissionMode.requireApproval =>
                      l10n.toolPermissionRequireApproval,
                    ToolPermissionMode.fullAccess =>
                      l10n.toolPermissionFullAccess,
                  }),
                  if (option == mode) ...[
                    const SizedBox(width: 16),
                    const Icon(Icons.check_rounded, size: 16),
                  ],
                ],
              ),
            ),
        ],
        builder: (context, controller, _) => SizedBox(
          width: 148,
          height: 36,
          child: OutlinedButton(
            onPressed: onSelected == null
                ? null
                : () => controller.isOpen
                      ? controller.close()
                      : controller.open(),
            style: _controlStyle(palette, width: 148, compact: compact),
            child: Row(
              children: [
                Icon(
                  Icons.admin_panel_settings_outlined,
                  size: 16,
                  color: palette.secondaryText,
                ),
                const SizedBox(width: 5),
                Expanded(
                  child: Text(
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
