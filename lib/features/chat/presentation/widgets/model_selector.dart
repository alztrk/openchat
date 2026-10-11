import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:openchat/app/openchat_dropdown.dart';
import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/domain/model_favorite.dart';
import 'package:openchat/features/chat/domain/model_pricing.dart';
import 'package:openchat/features/chat/data/model_pricing_repository.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

import 'package:openchat/features/chat/presentation/widgets/composer_control_style.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ModelSelector extends StatefulWidget {
  const ModelSelector({
    super.key,
    required this.label,
    required this.palette,
    required this.compact,
    this.borderless = false,
    this.width = 152,
    this.showReasoningSelector = false,
    this.reasoningLevel,
    this.reasoningOptions = const <String>[],
    this.onReasoningSelected,
    required this.models,
    required this.favoriteModels,
    required this.selectedModelId,
    required this.selectedModelRouteKey,
    required this.providerId,
    required this.isChatGptConnected,
    this.chatGptFastModeEnabled = false,
    this.chatGptFastModeAvailable = false,
    this.chatGptFastModeLoading = false,
    this.chatGptFastModeSaving = false,
    this.onChatGptFastModeChanged,
    this.availableProviderIds = const <String>{},
    this.hiddenModelKeys = const <String>{},
    this.pricingRepository,
    required this.onProviderSelected,
    required this.isLoadingModels,
    required this.emptyModelsLabel,
    required this.onSelected,
    required this.onFavoriteChanged,
    required this.onFavoriteSelected,
  });

  final String label;
  final OpenChatPalette palette;
  final bool compact;
  final bool borderless;
  final double width;
  final bool showReasoningSelector;
  final String? reasoningLevel;
  final List<String> reasoningOptions;
  final ValueChanged<String?>? onReasoningSelected;
  final List<ChatGptModel> models;
  final List<FavoriteModel> favoriteModels;
  final String? selectedModelId;
  final String providerId;
  final bool isChatGptConnected;
  final bool chatGptFastModeEnabled;
  final bool chatGptFastModeAvailable;
  final bool chatGptFastModeLoading;
  final bool chatGptFastModeSaving;
  final ValueChanged<bool>? onChatGptFastModeChanged;
  final Set<String> availableProviderIds;
  final Set<String> hiddenModelKeys;
  final ModelPricingRepository? pricingRepository;
  final ValueChanged<String>? onProviderSelected;
  final bool isLoadingModels;
  final String emptyModelsLabel;
  final String? selectedModelRouteKey;
  final ValueChanged<ChatGptModel>? onSelected;
  final void Function(
    String providerId,
    String modelId,
    String displayName,
    String? sourceConnectionId,
    bool isFavorite,
  )?
  onFavoriteChanged;
  final ValueChanged<FavoriteModel>? onFavoriteSelected;

  @override
  State<ModelSelector> createState() => _ModelSelectorState();
}

class _ModelSelectorState extends State<ModelSelector> {
  static const double _menuHeight = 420;
  static const Set<String> _localEngineProviderIds = <String>{
    'llama_cpp',
    'vllm',
    'exllama',
  };

  final _menuController = MenuController();
  final _menuBoundsKey = GlobalKey();
  final _searchController = TextEditingController();
  final _providerScrollController = ScrollController();
  final _modelScrollController = ScrollController();
  bool _showFavorites = false;
  String _searchQuery = '';

  @override
  void didUpdateWidget(covariant ModelSelector oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!_showFavorites && oldWidget.providerId != widget.providerId) {
      _clearSearch();
    }
  }

  @override
  void dispose() {
    _searchController.dispose();
    _providerScrollController.dispose();
    _modelScrollController.dispose();
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

  bool _isProviderAvailable(String providerId) => switch (providerId) {
    'chatgpt' =>
      widget.isChatGptConnected ||
          widget.availableProviderIds.contains(providerId),
    'opencode' => true,
    _ => widget.availableProviderIds.contains(providerId),
  };

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final isEnabled =
        widget.onSelected != null || widget.onProviderSelected != null;
    final availableModels = widget.models
        .where(
          (model) =>
              (model.isAvailable ||
                  _localEngineProviderIds.contains(model.providerId)) &&
              !widget.hiddenModelKeys.contains(model.routeKey) &&
              !widget.hiddenModelKeys.contains(
                '${model.providerId}:${model.id}',
              ),
        )
        .toList(growable: false);
    final scopedFavorites = widget.onProviderSelected == null
        ? widget.favoriteModels
              .where(
                (favorite) =>
                    _providerFamily(favorite.providerId) == widget.providerId,
              )
              .toList(growable: false)
        : widget.favoriteModels;
    final favoriteModels = scopedFavorites
        .where(
          (favorite) =>
              !widget.hiddenModelKeys.contains(
                '${favorite.providerId}:::${favorite.modelId}',
              ) &&
              !widget.hiddenModelKeys.contains(
                '${favorite.providerId}:${favorite.sourceConnectionId ?? ''}::${favorite.modelId}',
              ) &&
              !widget.hiddenModelKeys.contains(
                '${favorite.providerId}:${favorite.modelId}',
              ),
        )
        .toList(growable: false);
    final normalizedQuery = _searchQuery.trim().toLowerCase();
    bool matchesQuery(Iterable<String> values) =>
        normalizedQuery.isEmpty ||
        values.any((value) => value.toLowerCase().contains(normalizedQuery));
    final visibleFavorites = favoriteModels
        .where(
          (favorite) => matchesQuery([
            favorite.displayName,
            favorite.modelId,
            _providerLabel(favorite.providerId, l10n),
            favorite.sourceConnectionId ?? '',
          ]),
        )
        .toList(growable: false);
    final visibleModels = availableModels
        .where(
          (model) => matchesQuery([
            model.displayName,
            model.id,
            model.sourceLabel ?? '',
            model.groupId == 'api' ? l10n.modelSourceApi : '',
            model.groupId == 'oauth' ? l10n.modelSourceOAuth : '',
            if (model.description == 'paid') l10n.openCodePaidModel,
            if (model.description == 'free') l10n.openCodeFreeModel,
          ]),
        )
        .toList(growable: false);
    final modelEntries = _modelEntries(visibleModels, widget.providerId, l10n);
    final shownCount = _showFavorites
        ? visibleFavorites.length
        : modelEntries.length;
    final media = MediaQuery.of(context);
    final screenWidth = media.size.width;
    final menuHeight = math.min(
      _menuHeight,
      math.max(120.0, media.size.height - media.viewInsets.bottom - 48),
    );
    final menuWidth = math.max(0.0, math.min(520.0, screenWidth - 32));
    final providerWidth = math.min(
      menuWidth * 0.36,
      menuWidth < 420 ? 112.0 : 152.0,
    );
    final reasoningWidth = math.max(
      0.0,
      math.min(128.0, menuWidth - providerWidth - 25),
    );

    return OpenChatDropdown(
      palette: widget.palette,
      controller: _menuController,
      crossAxisUnconstrained: true,
      alignmentOffset: const Offset(0, 8),
      reservedPadding: const EdgeInsets.all(12),
      menuPadding: EdgeInsets.zero,
      maximumSize: Size(menuWidth, menuHeight),
      menuChildren: [
        KeyedSubtree(
          key: const ValueKey<String>('model-selector-menu'),
          child: SizedBox(
            key: _menuBoundsKey,
            width: menuWidth,
            height: menuHeight,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Container(
                  width: providerWidth,
                  color: widget.palette.navigation,
                  padding: const EdgeInsets.all(8),
                  child: Scrollbar(
                    controller: _providerScrollController,
                    thumbVisibility: true,
                    interactive: true,
                    thickness: 5,
                    radius: const Radius.circular(4),
                    child: SingleChildScrollView(
                      controller: _providerScrollController,
                      primary: false,
                      physics: const BouncingScrollPhysics(),
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
                          for (final providerId in _providerIds) ...[
                            _ModelProviderTab(
                              providerId: providerId,
                              label: _providerLabel(providerId, l10n),
                              selected:
                                  !_showFavorites &&
                                  widget.providerId == providerId,
                              onPressed:
                                  !_isProviderAvailable(providerId) ||
                                      widget.onProviderSelected == null
                                  ? null
                                  : () => _selectProvider(providerId),
                              palette: widget.palette,
                            ),
                            const SizedBox(height: 4),
                          ],
                        ],
                      ),
                    ),
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
                        Row(
                          children: [
                            if (widget.showReasoningSelector) ...[
                              _ReasoningControl(
                                palette: widget.palette,
                                level: widget.reasoningLevel,
                                options: widget.reasoningOptions,
                                availableWidth: reasoningWidth,
                                onSelected: widget.onReasoningSelected,
                              ),
                              const SizedBox(width: 4),
                            ],
                            Expanded(
                              child: ConstrainedBox(
                                constraints: const BoxConstraints(
                                  minHeight: 40,
                                ),
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
                                    labelText: l10n.searchModels,
                                    hintText: l10n.modelSearchHint,
                                    hintStyle: TextStyle(
                                      color: widget.palette.secondaryText,
                                      fontSize: 13,
                                    ),
                                    prefixIcon: Icon(
                                      LucideIcons.search,
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
                                  ),
                                ),
                              ),
                            ),
                            if (!_showFavorites &&
                                widget.providerId == 'chatgpt') ...[
                              const SizedBox(width: 4),
                              _ChatGptFastModeButton(
                                enabled: widget.chatGptFastModeEnabled,
                                available: widget.chatGptFastModeAvailable,
                                loading: widget.chatGptFastModeLoading,
                                saving: widget.chatGptFastModeSaving,
                                palette: widget.palette,
                                onChanged: widget.onChatGptFastModeChanged,
                              ),
                            ],
                          ],
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
                                      : SingleChildScrollView(
                                          primary: false,
                                          child: Padding(
                                            padding: const EdgeInsets.all(16),
                                            child: Text(
                                              _showFavorites
                                                  ? favoriteModels.isNotEmpty &&
                                                            normalizedQuery
                                                                .isNotEmpty
                                                        ? l10n.modelSearchNoResults
                                                        : l10n.favoriteModelsEmpty
                                                  : availableModels
                                                            .isNotEmpty &&
                                                        normalizedQuery
                                                            .isNotEmpty
                                                  ? l10n.modelSearchNoResults
                                                  : widget.emptyModelsLabel,
                                              textAlign: TextAlign.center,
                                              style: TextStyle(
                                                color: widget
                                                    .palette
                                                    .secondaryText,
                                                fontSize: 13,
                                              ),
                                            ),
                                          ),
                                        ),
                                )
                              : Scrollbar(
                                  controller: _modelScrollController,
                                  thumbVisibility: true,
                                  interactive: true,
                                  thickness: 5,
                                  radius: const Radius.circular(4),
                                  child: ListView.builder(
                                    controller: _modelScrollController,
                                    primary: false,
                                    physics: const BouncingScrollPhysics(),
                                    padding: const EdgeInsets.symmetric(
                                      vertical: 4,
                                    ),
                                    itemCount: shownCount,
                                    itemBuilder: (context, index) {
                                      if (_showFavorites) {
                                        final favorite =
                                            visibleFavorites[index];
                                        final favoriteModel =
                                            _findFavoriteModel(
                                              widget.models,
                                              favorite,
                                            );
                                        final providerLabel = _providerLabel(
                                          favorite.providerId,
                                          l10n,
                                        );
                                        return _ModelOption(
                                          key: ValueKey<String>(
                                            '${favorite.providerId}:${favorite.modelId}',
                                          ),
                                          menuBoundsKey: _menuBoundsKey,
                                          providerId: _providerFamily(
                                            favorite.providerId,
                                          ),
                                          title:
                                              '${favorite.displayName} · $providerLabel',
                                          description: _modelDescription(
                                            favoriteModel?.description,
                                          ),
                                          contextWindow:
                                              favoriteModel?.contextWindow,
                                          supportsImages:
                                              favoriteModel?.supportsImages ??
                                              false,
                                          supportsTools:
                                              favoriteModel?.supportsTools,
                                          modelId: favorite.modelId,
                                          providerLabel: providerLabel,
                                          pricingRepository:
                                              widget.pricingRepository,
                                          isAvailable:
                                              favoriteModel?.isAvailable ??
                                              true,
                                          selected:
                                              _providerFamily(
                                                    favorite.providerId,
                                                  ) ==
                                                  widget.providerId &&
                                              favorite.modelId ==
                                                  widget.selectedModelId &&
                                              (!_isApiKeyProvider(
                                                    favorite.providerId,
                                                  ) ||
                                                  widget.selectedModelRouteKey ==
                                                      '${favorite.providerId}:${favorite.sourceConnectionId ?? ''}::${favorite.modelId}'),
                                          isFavorite: true,
                                          palette: widget.palette,
                                          addFavoriteLabel:
                                              l10n.addModelFavorite,
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
                                                  favorite.sourceConnectionId,
                                                  false,
                                                ),
                                        );
                                      }

                                      final entry = modelEntries[index];
                                      if (entry case _ModelSectionEntry(
                                        :final label,
                                      )) {
                                        return _ModelSection(
                                          label: label,
                                          palette: widget.palette,
                                        );
                                      }
                                      final model =
                                          (entry as _ModelRowEntry).model;
                                      final isFavorite = widget.favoriteModels
                                          .any(
                                            (favorite) =>
                                                favorite.providerId ==
                                                    model.providerId &&
                                                favorite.modelId == model.id &&
                                                (!_isApiKeyProvider(
                                                      model.providerId,
                                                    ) ||
                                                    favorite.sourceConnectionId ==
                                                        model.connectionId),
                                          );
                                      final title = model.sourceLabel == null
                                          ? model.displayName
                                          : '${model.displayName} · ${model.sourceLabel}';
                                      return _ModelOption(
                                        key: ValueKey<String>(model.routeKey),
                                        menuBoundsKey: _menuBoundsKey,
                                        providerId: _providerFamily(
                                          model.providerId,
                                        ),
                                        title: title,
                                        description: _modelDescription(
                                          model.description ??
                                              _unavailableModelDescription(
                                                model,
                                                l10n,
                                              ),
                                        ),
                                        contextWindow: model.contextWindow,
                                        supportsImages: model.supportsImages,
                                        supportsTools: model.supportsTools,
                                        modelId: model.id,
                                        providerLabel: _providerLabel(
                                          model.providerId,
                                          l10n,
                                        ),
                                        pricingRepository:
                                            widget.pricingRepository,
                                        isAvailable: model.isAvailable,
                                        selected:
                                            model.routeKey ==
                                            widget.selectedModelRouteKey,
                                        isFavorite: isFavorite,
                                        palette: widget.palette,
                                        addFavoriteLabel: l10n.addModelFavorite,
                                        removeFavoriteLabel:
                                            l10n.removeModelFavorite,
                                        onSelected:
                                            !model.isAvailable ||
                                                widget.onSelected == null
                                            ? null
                                            : () {
                                                widget.onSelected!(model);
                                                _menuController.close();
                                              },
                                        onToggleFavorite:
                                            !model.isAvailable ||
                                                widget.onFavoriteChanged == null
                                            ? null
                                            : () => widget.onFavoriteChanged!(
                                                model.providerId,
                                                model.id,
                                                model.displayName,
                                                model.connectionId,
                                                !isFavorite,
                                              ),
                                      );
                                    },
                                  ),
                                ),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ],
      builder: (context, controller, _, focusNode) => Tooltip(
        message: widget.label,
        child: SizedBox(
          width: widget.width,
          height: composerControlHeight(context),
          child: OutlinedButton(
            focusNode: focusNode,
            onPressed:
                widget.onSelected == null && widget.onProviderSelected == null
                ? null
                : () => controller.isOpen
                      ? controller.close()
                      : controller.open(),
            style: composerControlStyle(
              widget.palette,
              width: widget.width,
              height: composerControlHeight(context),
              compact: widget.compact,
              leftPadding: 12,
              rightPadding: 12,
              borderless: widget.borderless,
            ),
            child: Row(
              children: [
                SizedBox(
                  width: 16,
                  height: 16,
                  child: Center(
                    child: ProviderIcon(
                      providerId: widget.providerId,
                      color: isEnabled
                          ? widget.palette.secondaryIcon
                          : widget.palette.disabledIcon,
                      size: 16,
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
                    style: Theme.of(context).textTheme.labelLarge?.copyWith(
                      fontSize: 13,
                      fontWeight: FontWeight.w500,
                      height: 18 / 13,
                    ),
                  ),
                ),
                SizedBox(
                  width: 16,
                  height: 16,
                  child: Center(
                    child: Icon(
                      controller.isOpen
                          ? LucideIcons.chevronUp
                          : LucideIcons.chevronDown,
                      size: 16,
                      color: isEnabled
                          ? widget.palette.secondaryIcon
                          : widget.palette.disabledIcon,
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

List<_ModelEntry> _modelEntries(
  List<ChatGptModel> models,
  String providerId,
  AppLocalizations l10n,
) {
  int compareServerSections(String left, String right) {
    final leftPort = int.tryParse(left.split(':').last) ?? 0;
    final rightPort = int.tryParse(right.split(':').last) ?? 0;
    final portComparison = leftPort.compareTo(rightPort);
    return portComparison == 0 ? left.compareTo(right) : portComparison;
  }

  final sections = <String, List<ChatGptModel>>{};
  for (final model in models) {
    sections.putIfAbsent(model.groupId ?? 'models', () => []).add(model);
  }
  final externalLlamaSections =
      sections.keys
          .where((section) => section.startsWith('external-llama-server:'))
          .toList()
        ..sort(compareServerSections);
  final managedLlamaSections =
      sections.keys
          .where((section) => section.startsWith('managed-llama-server:'))
          .toList()
        ..sort(compareServerSections);
  final order = switch (providerId) {
    'opencode' => const ['free', 'paid', 'models'],
    'openrouter' => const ['free', 'models'],
    'chatgpt' => const ['api', 'oauth', 'models'],
    'llama_cpp' => [
      'managed',
      ...managedLlamaSections,
      ...externalLlamaSections,
      'models',
    ],
    _ => const ['models'],
  };
  final entries = <_ModelEntry>[];
  for (final section in order) {
    final sectionModels = sections[section];
    if (sectionModels == null || sectionModels.isEmpty) continue;
    String? label;
    if (providerId == 'llama_cpp' && section == 'managed') {
      label = l10n.localEngineManagedModelSection;
    } else if (providerId == 'llama_cpp' &&
        section.startsWith('managed-llama-server:')) {
      final port = int.tryParse(section.split(':').last);
      if (port != null) {
        label = l10n.localEngineManagedServerModelSection(port);
      }
    } else if (providerId == 'llama_cpp' &&
        section.startsWith('external-llama-server:')) {
      final port = int.tryParse(section.split(':').last);
      if (port != null) {
        label = l10n.localEngineExternalModelSection(port);
      }
    } else {
      label = switch (section) {
        'api' => l10n.modelSourceApi,
        'oauth' => l10n.modelSourceOAuth,
        'free' => l10n.openCodeFreeModels,
        'paid' => l10n.openCodeApiModels,
        _ => null,
      };
    }
    if (label != null) entries.add(_ModelSectionEntry(label));
    entries.addAll(sectionModels.map(_ModelRowEntry.new));
  }
  return entries;
}

const _providerIds = <String>[
  'chatgpt',
  'opencode',
  'gemini',
  'groq',
  'cerebras',
  'openrouter',
  'mistral',
  'llama_cpp',
  'vllm',
  'exllama',
];

String _providerLabel(String providerId, AppLocalizations l10n) =>
    switch (providerId) {
      'chatgpt' || 'chatgpt_api' => l10n.chatGptProvider,
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

bool _isApiKeyProvider(String providerId) =>
    providerId == 'chatgpt_api' ||
    providerId == 'gemini' ||
    providerId == 'groq' ||
    providerId == 'cerebras' ||
    providerId == 'openrouter' ||
    providerId == 'mistral';

String _providerFamily(String providerId) =>
    providerId == 'chatgpt_api' ? 'chatgpt' : providerId;

class _ReasoningControl extends StatelessWidget {
  const _ReasoningControl({
    required this.palette,
    required this.level,
    required this.options,
    required this.availableWidth,
    required this.onSelected,
  });

  final OpenChatPalette palette;
  final String? level;
  final List<String> options;
  final double availableWidth;
  final ValueChanged<String?>? onSelected;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final selectedLabel = switch (level) {
      null => l10n.reasoningDefault,
      final selectedLevel => _reasoningLabel(l10n, selectedLevel),
    };
    final selector = OpenChatSelect<String?>(
      options: [
        OpenChatSelectOption<String?>(
          value: null,
          label: l10n.reasoningDefault,
        ),
        for (final option in options)
          OpenChatSelectOption<String?>(
            value: option,
            label: _reasoningLabel(l10n, option),
          ),
      ],
      value: level,
      onChanged: onSelected,
      palette: palette,
      width: math.min(128, availableWidth),
      menuWidth: math.min(260, math.max(0, availableWidth)),
      height: composerControlHeight(context),
      horizontalPadding: 6,
      borderless: true,
      selectedContent: Row(
        children: [
          Icon(LucideIcons.brain, size: 16, color: palette.secondaryIcon),
          const SizedBox(width: 5),
          Flexible(
            child: Text(
              selectedLabel,
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
    );

    return selector;
  }
}

String _reasoningLabel(AppLocalizations l10n, String value) {
  return switch (value.trim().toLowerCase()) {
    'minimal' => l10n.reasoningMinimal,
    'low' => l10n.reasoningLow,
    'medium' => l10n.reasoningMedium,
    'high' => l10n.reasoningHigh,
    'xhigh' || 'extra_high' || 'extra-high' => l10n.reasoningExtraHigh,
    'max' || 'maximum' => l10n.reasoningMax,
    'ultra' => l10n.reasoningUltra,
    _ => value,
  };
}

sealed class _ModelEntry {
  const _ModelEntry();
}

class _ModelSectionEntry extends _ModelEntry {
  const _ModelSectionEntry(this.label);

  final String label;
}

class _ModelRowEntry extends _ModelEntry {
  const _ModelRowEntry(this.model);

  final ChatGptModel model;
}

class _ModelSection extends StatelessWidget {
  const _ModelSection({required this.label, required this.palette});

  final String label;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => SizedBox(
    height: 30,
    child: Align(
      alignment: Alignment.centerLeft,
      child: Text(
        label,
        style: TextStyle(
          color: palette.secondaryText,
          fontSize: OpenChatTypography.metadata,
          fontWeight: FontWeight.w600,
        ),
      ),
    ),
  );
}

class _ModelOption extends StatefulWidget {
  const _ModelOption({
    required this.menuBoundsKey,
    required this.providerId,
    required this.title,
    required this.description,
    required this.contextWindow,
    required this.supportsImages,
    required this.supportsTools,
    required this.modelId,
    required this.providerLabel,
    required this.pricingRepository,
    required this.isAvailable,
    required this.selected,
    required this.isFavorite,
    required this.palette,
    required this.addFavoriteLabel,
    required this.removeFavoriteLabel,
    required this.onSelected,
    required this.onToggleFavorite,
    super.key,
  });

  final GlobalKey menuBoundsKey;
  final String providerId;
  final String title;
  final String? description;
  final int? contextWindow;
  final bool supportsImages;
  final bool? supportsTools;
  final String modelId;
  final String providerLabel;
  final ModelPricingRepository? pricingRepository;
  final bool isAvailable;
  final bool selected;
  final bool isFavorite;
  final OpenChatPalette palette;
  final String addFavoriteLabel;
  final String removeFavoriteLabel;
  final VoidCallback? onSelected;
  final VoidCallback? onToggleFavorite;

  @override
  State<_ModelOption> createState() => _ModelOptionState();
}

class _ModelOptionState extends State<_ModelOption> {
  static const _localProviders = <String>{'llama_cpp', 'vllm', 'exllama'};

  final _detailsOverlayController = OverlayPortalController();
  final _rowFocusNode = FocusNode();
  Timer? _tooltipCloseTimer;
  bool _hovered = false;
  bool _detailsHovered = false;
  bool _rowFocused = false;
  bool _favoriteFocused = false;
  bool _pricingRequested = false;
  bool _pricingLoading = false;
  bool _pricingFailed = false;
  int _pricingGeneration = 0;
  ModelPricingSnapshot? _pricingSnapshot;

  @override
  void didUpdateWidget(covariant _ModelOption oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.providerId != widget.providerId ||
        oldWidget.modelId != widget.modelId) {
      _pricingGeneration++;
      _pricingSnapshot = null;
      _pricingRequested = false;
      _pricingLoading = false;
      _pricingFailed = false;
    }
  }

  @override
  void dispose() {
    _tooltipCloseTimer?.cancel();
    _pricingGeneration++;
    _rowFocusNode.dispose();
    super.dispose();
  }

  void _showDetails() {
    _tooltipCloseTimer?.cancel();
    _detailsOverlayController.show();
    if (_pricingRequested ||
        widget.pricingRepository == null ||
        _localProviders.contains(widget.providerId)) {
      return;
    }
    _pricingRequested = true;
    unawaited(_loadPricing());
  }

  void _scheduleHideDetails() {
    _tooltipCloseTimer?.cancel();
    _tooltipCloseTimer = Timer(const Duration(milliseconds: 180), () {
      if (mounted && !_hovered && !_detailsHovered && !_rowFocusNode.hasFocus) {
        _detailsOverlayController.hide();
      }
    });
  }

  ({double left, double top, double width, double height})? _detailsPosition(
    BuildContext context,
    Size overlaySize,
  ) {
    final menuRenderObject = widget.menuBoundsKey.currentContext
        ?.findRenderObject();
    final overlayRenderObject = Overlay.of(
      context,
      rootOverlay: true,
    ).context.findRenderObject();
    if (menuRenderObject is! RenderBox ||
        overlayRenderObject is! RenderBox ||
        !menuRenderObject.hasSize ||
        !overlayRenderObject.hasSize) {
      return null;
    }

    final overlayOrigin = overlayRenderObject.localToGlobal(Offset.zero);
    final menuOrigin =
        menuRenderObject.localToGlobal(Offset.zero) - overlayOrigin;
    final panelWidth = math.min(320.0, overlaySize.width - 32);
    final panelHeight = math.min(
      menuRenderObject.size.height,
      overlaySize.height - 32,
    );
    final rightPosition = menuOrigin.dx + menuRenderObject.size.width + 12;
    final leftPosition = menuOrigin.dx - panelWidth - 12;
    final left = rightPosition + panelWidth <= overlaySize.width - 16
        ? rightPosition
        : leftPosition >= 16
        ? leftPosition
        : (overlaySize.width - panelWidth) / 2;
    final top = menuOrigin.dy
        .clamp(16.0, math.max(16.0, overlaySize.height - panelHeight - 16))
        .toDouble();
    return (left: left, top: top, width: panelWidth, height: panelHeight);
  }

  Future<void> _loadPricing() async {
    final repository = widget.pricingRepository;
    if (repository == null) return;
    final generation = ++_pricingGeneration;
    setState(() {
      _pricingLoading = true;
      _pricingFailed = false;
    });
    try {
      final result = await repository.load(
        providerId: widget.providerId,
        modelId: widget.modelId,
      );
      if (!mounted || generation != _pricingGeneration) return;
      setState(() {
        _pricingSnapshot = result;
        _pricingLoading = false;
      });
    } on OpenChatServiceException {
      if (!mounted || generation != _pricingGeneration) return;
      setState(() {
        _pricingFailed = true;
        _pricingLoading = false;
      });
    } on FormatException {
      if (!mounted || generation != _pricingGeneration) return;
      setState(() {
        _pricingFailed = true;
        _pricingLoading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final contextWindow = widget.contextWindow;
    final contextWindowLabel = contextWindow != null && contextWindow > 0
        ? _formatContextWindow(contextWindow)
        : null;
    final hasDetails = widget.description != null || contextWindowLabel != null;
    final l10n = context.openchatL10n;
    final touchTargets = switch (Theme.of(context).platform) {
      TargetPlatform.android ||
      TargetPlatform.iOS ||
      TargetPlatform.fuchsia => true,
      _ => false,
    };
    final showFavorite =
        touchTargets ||
        _hovered ||
        _rowFocused ||
        _favoriteFocused ||
        widget.isFavorite;
    return OverlayPortal(
      controller: _detailsOverlayController,
      overlayChildBuilder: (context) => Positioned.fill(
        child: LayoutBuilder(
          builder: (context, constraints) {
            final position = _detailsPosition(context, constraints.biggest);
            if (position == null) return const SizedBox.shrink();
            return Stack(
              children: [
                Positioned(
                  left: position.left,
                  top: position.top,
                  width: position.width,
                  height: position.height,
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: MouseRegion(
                      onEnter: (_) {
                        _tooltipCloseTimer?.cancel();
                        setState(() => _detailsHovered = true);
                      },
                      onExit: (_) {
                        setState(() => _detailsHovered = false);
                        _scheduleHideDetails();
                      },
                      child: Material(
                        key: const ValueKey<String>('model-details-panel'),
                        color: widget.palette.surface,
                        elevation: 12,
                        shadowColor: Colors.black.withValues(alpha: 0.3),
                        clipBehavior: Clip.antiAlias,
                        shape: RoundedRectangleBorder(
                          side: BorderSide(color: widget.palette.border),
                          borderRadius: BorderRadius.circular(
                            OpenChatRadii.panel,
                          ),
                        ),
                        child: Padding(
                          padding: const EdgeInsets.all(12),
                          child: _ModelDetailsPopover(
                            title: widget.title,
                            providerLabel:
                                _pricingSnapshot?.model?.providerName ??
                                widget.providerLabel,
                            description: widget.description,
                            contextWindow:
                                _pricingSnapshot?.model?.contextWindow ??
                                widget.contextWindow,
                            maximumOutput:
                                _pricingSnapshot?.model?.maximumOutput,
                            supportsImages: widget.supportsImages,
                            supportsTools:
                                _pricingSnapshot?.model?.supportsTools ??
                                widget.supportsTools,
                            inputModalities:
                                _pricingSnapshot?.model?.inputModalities ??
                                const <String>[],
                            outputModalities:
                                _pricingSnapshot?.model?.outputModalities ??
                                const <String>[],
                            pricing: _pricingSnapshot?.model?.pricing,
                            pricingState: _pricingSnapshot?.state,
                            pricingLoading: _pricingLoading,
                            pricingLoadFailed: _pricingFailed,
                            pricingUnavailable:
                                _pricingSnapshot != null &&
                                _pricingSnapshot?.model?.pricing == null,
                            palette: widget.palette,
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ],
            );
          },
        ),
      ),
      child: MouseRegion(
        onEnter: (_) {
          setState(() => _hovered = true);
          _showDetails();
        },
        onExit: (_) {
          setState(() => _hovered = false);
          _scheduleHideDetails();
        },
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: widget.selected
                ? widget.palette.selected
                : _rowFocused || _hovered
                ? widget.palette.hover
                : Colors.transparent,
            borderRadius: BorderRadius.circular(OpenChatRadii.button),
            border: widget.selected
                ? Border.all(color: widget.palette.accent)
                : null,
          ),
          child: Row(
            children: [
              Expanded(
                child: Semantics(
                  selected: widget.selected,
                  child: InkWell(
                    onTap: widget.onSelected,
                    focusNode: _rowFocusNode,
                    onFocusChange: (focused) {
                      setState(() => _rowFocused = focused);
                      if (focused) _showDetails();
                    },
                    borderRadius: BorderRadius.circular(OpenChatRadii.button),
                    child: ConstrainedBox(
                      constraints: BoxConstraints(
                        minHeight: hasDetails ? 56 : 44,
                      ),
                      child: Padding(
                        padding: const EdgeInsets.only(
                          left: 12,
                          right: 4,
                          top: 6,
                          bottom: 6,
                        ),
                        child: Row(
                          children: [
                            ProviderIcon(
                              providerId: widget.providerId,
                              color: widget.isAvailable
                                  ? widget.palette.secondaryIcon
                                  : widget.palette.disabledIcon,
                              size: 16,
                            ),
                            if (widget.selected) ...[
                              const SizedBox(width: 4),
                              Icon(
                                LucideIcons.check,
                                size: 16,
                                color: widget.palette.accent,
                              ),
                              const SizedBox(width: 4),
                            ] else
                              const SizedBox(width: 8),
                            Expanded(
                              child: Column(
                                mainAxisAlignment: MainAxisAlignment.center,
                                crossAxisAlignment: CrossAxisAlignment.start,
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  Tooltip(
                                    message: widget.title,
                                    child: Text(
                                      widget.title,
                                      maxLines: 2,
                                      overflow: TextOverflow.ellipsis,
                                      style: TextStyle(
                                        color: widget.isAvailable
                                            ? widget.palette.text
                                            : widget.palette.secondaryText,
                                        fontSize: 13,
                                      ),
                                    ),
                                  ),
                                  if (widget.description
                                      case final description?)
                                    Padding(
                                      padding: const EdgeInsets.only(top: 2),
                                      child: Text(
                                        description,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: TextStyle(
                                          color: widget.palette.secondaryText,
                                          fontSize: OpenChatTypography.metadata,
                                        ),
                                      ),
                                    ),
                                  if (contextWindowLabel case final label?)
                                    Padding(
                                      padding: const EdgeInsets.only(top: 3),
                                      child: Tooltip(
                                        message: widget.providerId == 'opencode'
                                            ? l10n.openCodeModelContextWindow(
                                                label,
                                              )
                                            : l10n.modelContextWindow(label),
                                        child: Text(
                                          label,
                                          style: TextStyle(
                                            color: widget.palette.secondaryText,
                                            fontSize:
                                                OpenChatTypography.metadata,
                                            fontWeight: FontWeight.w600,
                                          ),
                                        ),
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
              ),
              if (widget.onToggleFavorite != null || widget.isFavorite)
                Focus(
                  onFocusChange: (focused) {
                    setState(() => _favoriteFocused = focused);
                    if (focused) {
                      _showDetails();
                    } else if (!_hovered && !_rowFocusNode.hasFocus) {
                      _scheduleHideDetails();
                    }
                  },
                  child: AnimatedOpacity(
                    opacity: showFavorite ? 1 : 0,
                    duration: MediaQuery.disableAnimationsOf(context)
                        ? Duration.zero
                        : const Duration(milliseconds: 100),
                    child: SizedBox(
                      width: touchTargets ? 44 : 40,
                      child: IconButton(
                        tooltip: widget.isFavorite
                            ? widget.removeFavoriteLabel
                            : widget.addFavoriteLabel,
                        onPressed: showFavorite
                            ? widget.onToggleFavorite
                            : null,
                        visualDensity: VisualDensity.standard,
                        padding: EdgeInsets.zero,
                        iconSize: 18,
                        color: widget.isFavorite
                            ? widget.palette.accent
                            : widget.onToggleFavorite == null
                            ? widget.palette.disabledIcon
                            : widget.palette.secondaryIcon,
                        icon: Icon(
                          widget.isFavorite
                              ? LucideIcons.star
                              : LucideIcons.starOff,
                        ),
                      ),
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

class _ModelDetailsPopover extends StatelessWidget {
  const _ModelDetailsPopover({
    required this.title,
    required this.providerLabel,
    required this.description,
    required this.contextWindow,
    required this.maximumOutput,
    required this.supportsImages,
    required this.supportsTools,
    required this.inputModalities,
    required this.outputModalities,
    required this.pricing,
    required this.pricingState,
    required this.pricingLoading,
    required this.pricingLoadFailed,
    required this.pricingUnavailable,
    required this.palette,
  });

  final String title;
  final String providerLabel;
  final String? description;
  final int? contextWindow;
  final int? maximumOutput;
  final bool supportsImages;
  final bool? supportsTools;
  final List<String> inputModalities;
  final List<String> outputModalities;
  final ModelTokenPricing? pricing;
  final String? pricingState;
  final bool pricingLoading;
  final bool pricingLoadFailed;
  final bool pricingUnavailable;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final locale = Localizations.localeOf(context).toString();
    final labels = <(String, String)>[
      (l10n.modelDetailsProvider, providerLabel),
      if (contextWindow case final value? when value > 0)
        (l10n.modelDetailsContext, _formatContextWindow(value)),
      if (maximumOutput case final value? when value > 0)
        (l10n.modelDetailsMaximumOutput, _formatContextWindow(value)),
      if (inputModalities.isNotEmpty)
        (l10n.modelDetailsInputModalities, inputModalities.join(', ')),
      if (outputModalities.isNotEmpty)
        (l10n.modelDetailsOutputModalities, outputModalities.join(', ')),
      if (supportsImages) (l10n.modelDetailsImages, l10n.modelDetailsSupported),
      if (supportsTools case final hasToolCalling?)
        (
          l10n.modelDetailsToolCalling,
          hasToolCalling
              ? l10n.modelDetailsSupported
              : l10n.modelDetailsUnsupported,
        ),
    ];
    final formatter = NumberFormat('0.######', locale);
    String price(double value) =>
        l10n.modelDetailsPerMillion('\$${formatter.format(value)}');
    final pricingRows = <(String, String)>[];
    if (pricing case final rates?) {
      pricingRows.add((l10n.modelDetailsInputPrice, price(rates.input)));
      pricingRows.add((l10n.modelDetailsOutputPrice, price(rates.output)));
      if (rates.reasoning case final value?) {
        pricingRows.add((l10n.modelDetailsReasoningPrice, price(value)));
      }
      if (rates.cacheRead case final value?) {
        pricingRows.add((l10n.modelDetailsCacheReadPrice, price(value)));
      }
      if (rates.cacheWrite case final value?) {
        pricingRows.add((l10n.modelDetailsCacheWritePrice, price(value)));
      }
      for (final tier in rates.tiers) {
        pricingRows.add((
          l10n.modelDetailsTier(_formatContextWindow(tier.contextWindow)),
          l10n.modelDetailsInputOutputPrices(
            price(tier.input),
            price(tier.output),
          ),
        ));
      }
    }

    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 300, maxHeight: 440),
      child: SingleChildScrollView(
        child: SizedBox(
          width: 272,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                title,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.titleSmall?.copyWith(
                  color: palette.text,
                  fontWeight: FontWeight.w600,
                ),
              ),
              if (description case final value?) ...[
                const SizedBox(height: 4),
                Text(
                  value,
                  maxLines: 3,
                  overflow: TextOverflow.ellipsis,
                  style: Theme.of(context).textTheme.bodySmall
                      ?.copyWith(color: palette.secondaryText),
                ),
              ],
              const SizedBox(height: 10),
              for (final row in labels)
                _ModelDetailRow(label: row.$1, value: row.$2),
              if (pricingLoading ||
                  pricingLoadFailed ||
                  pricingUnavailable ||
                  pricingRows.isNotEmpty) ...[
                const SizedBox(height: 8),
                Divider(height: 1, color: palette.border),
                const SizedBox(height: 8),
                Text(
                  l10n.modelDetailsPricing,
                  style: Theme.of(context).textTheme.labelMedium?.copyWith(
                    color: palette.text,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 5),
                if (pricingLoading)
                  Text(
                    l10n.modelDetailsPricingLoading,
                    style: Theme.of(context).textTheme.bodySmall
                        ?.copyWith(color: palette.secondaryText),
                  )
                else if (pricingLoadFailed)
                  Text(
                    l10n.modelDetailsPricingLoadFailed,
                    style: Theme.of(context).textTheme.bodySmall
                        ?.copyWith(color: palette.secondaryText),
                  )
                else if (pricingUnavailable || pricingRows.isEmpty)
                  Text(
                    l10n.modelDetailsPricingUnavailable,
                    style: Theme.of(context).textTheme.bodySmall
                        ?.copyWith(color: palette.secondaryText),
                  )
                else ...[
                  for (final row in pricingRows)
                    _ModelDetailRow(label: row.$1, value: row.$2),
                  const SizedBox(height: 5),
                  Text(
                    pricingState == 'stale'
                        ? l10n.modelDetailsPricingStale
                        : l10n.modelDetailsPricingSource,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: palette.secondaryText,
                      fontSize: OpenChatTypography.metadata,
                    ),
                  ),
                ],
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _ModelDetailRow extends StatelessWidget {
  const _ModelDetailRow({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Text(
              label,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
            ),
          ),
          const SizedBox(width: 12),
          Flexible(
            child: Text(
              value,
              textAlign: TextAlign.end,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.text, fontWeight: FontWeight.w500),
            ),
          ),
        ],
      ),
    );
  }
}

ChatGptModel? _findFavoriteModel(
  List<ChatGptModel> models,
  FavoriteModel favorite,
) {
  for (final model in models) {
    if (model.providerId == favorite.providerId &&
        model.id == favorite.modelId &&
        (!_isApiKeyProvider(favorite.providerId) ||
            model.connectionId == favorite.sourceConnectionId)) {
      return model;
    }
  }
  return null;
}

String? _modelDescription(String? description) {
  final value = description?.trim();
  return value == null || value.isEmpty ? null : value;
}

String? _unavailableModelDescription(
  ChatGptModel model,
  AppLocalizations l10n,
) {
  if (model.isAvailable ||
      !_ModelSelectorState._localEngineProviderIds.contains(model.providerId)) {
    return null;
  }
  return switch (model.unavailabilityReason) {
    'model_file_missing' => l10n.localModelPathMissing,
    'local_engine_not_ready' => l10n.localModelEngineNotReady,
    _ => l10n.localModelEngineNotReady,
  };
}

String _formatContextWindow(int tokens) {
  if (tokens >= 1_000_000) {
    final millions = tokens / 1_000_000;
    final formatted = millions
        .toStringAsFixed(2)
        .replaceFirst(RegExp(r'0+$'), '')
        .replaceFirst(RegExp(r'\.$'), '');
    return '${formatted}M';
  }
  if (tokens >= 1_000) {
    final thousands = tokens / 1_000;
    final formatted = thousands == thousands.roundToDouble()
        ? thousands.toStringAsFixed(0)
        : thousands.toStringAsFixed(1);
    return '${formatted}K';
  }
  return tokens.toString();
}

class _ChatGptFastModeButton extends StatelessWidget {
  const _ChatGptFastModeButton({
    required this.enabled,
    required this.available,
    required this.loading,
    required this.saving,
    required this.palette,
    required this.onChanged,
  });

  final bool enabled;
  final bool available;
  final bool loading;
  final bool saving;
  final OpenChatPalette palette;
  final ValueChanged<bool>? onChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final canChange = available && !loading && !saving && onChanged != null;
    final isActive = available && enabled;
    final tooltip = switch ((loading, available, enabled)) {
      (true, _, _) => l10n.chatGptFastModeLoadingTooltip,
      (_, false, _) => l10n.chatGptFastModeUnavailableTooltip,
      (_, _, true) => l10n.chatGptFastModeEnabledTooltip,
      _ => l10n.chatGptFastModeDisabledTooltip,
    };

    return SizedBox.square(
      dimension: 44,
      child: IconButton(
        tooltip: tooltip,
        onPressed: canChange ? () => onChanged?.call(!enabled) : null,
        style: IconButton.styleFrom(
          foregroundColor: isActive
              ? palette.accentIcon
              : palette.secondaryIcon,
          disabledForegroundColor: palette.disabledIcon,
          backgroundColor: isActive
              ? palette.accent.withValues(alpha: 0.14)
              : Colors.transparent,
          minimumSize: const Size.square(44),
          padding: EdgeInsets.zero,
          tapTargetSize: MaterialTapTargetSize.shrinkWrap,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(OpenChatRadii.button),
          ),
        ),
        icon: const Icon(LucideIcons.zap, size: 18),
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
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      selected: selected,
      child: Tooltip(
        message: label,
        child: TextButton(
          onPressed: onPressed,
          style: TextButton.styleFrom(
            alignment: Alignment.centerLeft,
            backgroundColor: selected ? palette.selected : Colors.transparent,
            foregroundColor: selected ? palette.text : palette.secondaryText,
            disabledForegroundColor: selected
                ? palette.text
                : palette.disabledForeground,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            minimumSize: const Size(0, 40),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(OpenChatRadii.button),
            ),
          ),
          child: Row(
            children: [
              SizedBox(
                width: 18,
                height: 18,
                child: Center(
                  child: ProviderIcon(
                    providerId: providerId,
                    color: selected
                        ? palette.secondaryIcon
                        : onPressed == null
                        ? palette.disabledIcon
                        : palette.secondaryText,
                    size: 18,
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
      ),
    );
  }
}
