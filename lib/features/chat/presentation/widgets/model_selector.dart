import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_dropdown.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/domain/model_favorite.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

import 'package:openchat/features/chat/presentation/widgets/composer_control_style.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';

class ModelSelector extends StatefulWidget {
  const ModelSelector({
    super.key,
    required this.label,
    required this.iconRoot,
    required this.palette,
    required this.compact,
    required this.models,
    required this.favoriteModels,
    required this.selectedModelId,
    required this.selectedModelRouteKey,
    required this.providerId,
    required this.isChatGptConnected,
    this.availableProviderIds = const <String>{},
    this.hiddenModelKeys = const <String>{},
    required this.onProviderSelected,
    required this.isLoadingModels,
    required this.emptyModelsLabel,
    required this.onSelected,
    required this.onFavoriteChanged,
    required this.onFavoriteSelected,
  });

  final String label;
  final String iconRoot;
  final OpenChatPalette palette;
  final bool compact;
  final List<ChatGptModel> models;
  final List<FavoriteModel> favoriteModels;
  final String? selectedModelId;
  final String providerId;
  final bool isChatGptConnected;
  final Set<String> availableProviderIds;
  final Set<String> hiddenModelKeys;
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
  final _searchController = TextEditingController();
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
    final screenWidth = MediaQuery.sizeOf(context).width;
    final menuWidth = math.max(0.0, math.min(520.0, screenWidth - 32));
    final providerWidth = math.min(
      menuWidth * 0.36,
      menuWidth < 420 ? 112.0 : 152.0,
    );

    return OpenChatDropdown(
      palette: widget.palette,
      controller: _menuController,
      crossAxisUnconstrained: true,
      alignmentOffset: const Offset(0, 8),
      reservedPadding: const EdgeInsets.all(12),
      menuPadding: EdgeInsets.zero,
      maximumSize: Size(menuWidth, _menuHeight),
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
                child: SingleChildScrollView(
                  primary: false,
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
                                primary: false,
                                padding: const EdgeInsets.symmetric(
                                  vertical: 4,
                                ),
                                itemCount: shownCount,
                                itemBuilder: (context, index) {
                                  if (_showFavorites) {
                                    final favorite = visibleFavorites[index];
                                    final favoriteModel = _findFavoriteModel(
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
                                      isAvailable:
                                          favoriteModel?.isAvailable ?? true,
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
                                  final model = (entry as _ModelRowEntry).model;
                                  final isFavorite = widget.favoriteModels.any(
                                    (favorite) =>
                                        favorite.providerId ==
                                            model.providerId &&
                                        favorite.modelId == model.id &&
                                        (!_isApiKeyProvider(model.providerId) ||
                                            favorite.sourceConnectionId ==
                                                model.connectionId),
                                  );
                                  final title = model.sourceLabel == null
                                      ? model.displayName
                                      : '${model.displayName} · ${model.sourceLabel}';
                                  return _ModelOption(
                                    key: ValueKey<String>(model.routeKey),
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
          style: composerControlStyle(
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
                    colorFilter: ColorFilter.mode(
                      isEnabled
                          ? widget.palette.secondaryIcon
                          : widget.palette.disabledIcon,
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
          fontSize: 11,
          fontWeight: FontWeight.w600,
        ),
      ),
    ),
  );
}

class _ModelOption extends StatefulWidget {
  const _ModelOption({
    required this.providerId,
    required this.title,
    required this.description,
    required this.contextWindow,
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

  final String providerId;
  final String title;
  final String? description;
  final int? contextWindow;
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
  bool _hovered = false;
  bool _rowFocused = false;
  bool _favoriteFocused = false;

  @override
  Widget build(BuildContext context) {
    final contextWindow = widget.contextWindow;
    final contextWindowLabel = contextWindow != null && contextWindow > 0
        ? _formatContextWindow(contextWindow)
        : null;
    final hasDetails = widget.description != null || contextWindowLabel != null;
    final l10n = context.openchatL10n;
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
                child: ConstrainedBox(
                  constraints: BoxConstraints(minHeight: hasDetails ? 56 : 44),
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
                          color: widget.palette.secondaryIcon,
                          size: 16,
                        ),
                        if (widget.selected) ...[
                          const SizedBox(width: 4),
                          Icon(
                            Icons.check_rounded,
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
                              Text(
                                widget.title,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: TextStyle(
                                  color: widget.isAvailable
                                      ? widget.palette.text
                                      : widget.palette.secondaryText,
                                  fontSize: 13,
                                ),
                              ),
                              if (widget.description case final description?)
                                Padding(
                                  padding: const EdgeInsets.only(top: 2),
                                  child: Text(
                                    description,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: TextStyle(
                                      color: widget.palette.secondaryText,
                                      fontSize: 11,
                                    ),
                                  ),
                                ),
                            ],
                          ),
                        ),
                        if (contextWindowLabel case final label?)
                          Padding(
                            padding: const EdgeInsets.only(left: 8, right: 4),
                            child: Tooltip(
                              message: widget.providerId == 'opencode'
                                  ? l10n.openCodeModelContextWindow(label)
                                  : l10n.modelContextWindow(label),
                              child: DecoratedBox(
                                decoration: BoxDecoration(
                                  color: widget.palette.hover,
                                  border: Border.all(
                                    color: widget.palette.border,
                                  ),
                                  borderRadius: BorderRadius.circular(7),
                                ),
                                child: Padding(
                                  padding: const EdgeInsets.symmetric(
                                    horizontal: 7,
                                    vertical: 3,
                                  ),
                                  child: Text(
                                    label,
                                    style: TextStyle(
                                      color: widget.palette.secondaryText,
                                      fontSize: 11,
                                      fontWeight: FontWeight.w600,
                                    ),
                                  ),
                                ),
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
                        : widget.onToggleFavorite == null
                        ? widget.palette.disabledIcon
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
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
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
    );
  }
}
