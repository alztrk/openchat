import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../../../app/openchat_theme.dart';
import '../../../../l10n/openchat_localizations.dart';
import '../../domain/chatgpt_connection.dart';
import '../../domain/model_favorite.dart';
import 'composer_control_style.dart';
import 'provider_icon.dart';

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
  final OpenChatPalette palette;
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
  State<ModelSelector> createState() => _ModelSelectorState();
}

class _ModelSelectorState extends State<ModelSelector> {
  static const double _menuHeight = 420;

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

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
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
                    color: widget.palette.secondaryIcon,
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
                child: ProviderIcon(
                  providerId: providerId,
                  color: selected
                      ? palette.secondaryIcon
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
