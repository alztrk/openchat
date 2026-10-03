import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:intl/intl.dart';

import 'package:openchat/app/local_engine_icon.dart';
import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/safe_markdown.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_surface_card.dart';
import 'package:openchat/features/models/data/hugging_face_models_repository.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ModelsPage extends StatefulWidget {
  const ModelsPage({
    required this.serviceClient,
    required this.downloadController,
    super.key,
  });

  final OpenChatServiceClient? serviceClient;
  final HuggingFaceDownloadController? downloadController;

  @override
  State<ModelsPage> createState() => _ModelsPageState();
}

class _ModelsPageState extends State<ModelsPage> {
  final TextEditingController _searchController = TextEditingController();
  HuggingFaceModelFormat _format = HuggingFaceModelFormat.gguf;
  HuggingFaceModelSort _sort = HuggingFaceModelSort.downloads;
  List<HuggingFaceModelSearchResult> _models = const [];
  HuggingFaceModelSearchResult? _selectedModel;
  HuggingFaceModelDetails? _details;
  String? _selectedGroupId;
  String? _pageError;
  String? _nextCursor;
  String? _failedPageCursor;
  List<String?> _pageCursors = <String?>[null];
  int? _failedPageIndex;
  int _pageIndex = 0;
  bool _isSearching = false;
  bool _isLoadingDetails = false;
  Timer? _searchDebounce;
  int _searchGeneration = 0;
  int _detailsGeneration = 0;
  final Set<String> _downloadedComponentKeys = <String>{};
  final Map<String, int> _visibleComponentCounts = <String, int>{};

  HuggingFaceModelsRepository? get _repository {
    final serviceClient = widget.serviceClient;
    return serviceClient == null
        ? null
        : HuggingFaceModelsRepository(serviceClient);
  }

  @override
  void initState() {
    super.initState();
    widget.downloadController?.addListener(_onDownloadChanged);
    unawaited(_searchModels());
  }

  @override
  void didUpdateWidget(covariant ModelsPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.downloadController != widget.downloadController) {
      oldWidget.downloadController?.removeListener(_onDownloadChanged);
      widget.downloadController?.addListener(_onDownloadChanged);
    }
    if (oldWidget.serviceClient != widget.serviceClient) {
      unawaited(_searchModels());
    }
  }

  @override
  void dispose() {
    _searchDebounce?.cancel();
    _searchController.dispose();
    widget.downloadController?.removeListener(_onDownloadChanged);
    super.dispose();
  }

  void _onDownloadChanged() {
    if (mounted) setState(() {});
  }

  Future<void> _searchModels() async {
    await _loadPage(cursor: null, pageIndex: 0, resetPagination: true);
  }

  Future<void> _loadPage({
    required String? cursor,
    required int pageIndex,
    required bool resetPagination,
  }) async {
    _searchDebounce?.cancel();
    final repository = _repository;
    if (repository == null) {
      setState(() {
        _isSearching = false;
        _models = const [];
        _nextCursor = null;
        _pageCursors = <String?>[null];
        _pageIndex = 0;
        _pageError = context.openchatL10n.modelSearchFailed;
      });
      return;
    }

    final generation = ++_searchGeneration;
    _detailsGeneration++;
    setState(() {
      _isSearching = true;
      _pageError = null;
      _selectedModel = null;
      _details = null;
      _selectedGroupId = null;
      _failedPageCursor = null;
      _failedPageIndex = null;
      if (resetPagination) {
        _models = const [];
        _nextCursor = null;
        _pageCursors = <String?>[null];
        _pageIndex = 0;
      }
    });
    try {
      final page = await repository.search(
        query: _searchController.text,
        format: _format,
        sort: _sort,
        cursor: cursor,
      );
      if (!mounted || generation != _searchGeneration) return;
      setState(() {
        _models = page.models;
        _nextCursor = page.nextCursor;
        _pageIndex = pageIndex;
        if (pageIndex == _pageCursors.length) {
          _pageCursors.add(cursor);
        } else if (pageIndex < _pageCursors.length) {
          _pageCursors[pageIndex] = cursor;
        }
        _isSearching = false;
      });
    } on OpenChatServiceException catch (error) {
      if (!mounted || generation != _searchGeneration) return;
      setState(() {
        _pageError = _messageForHubError(error);
        _failedPageCursor = cursor;
        _failedPageIndex = pageIndex;
        _isSearching = false;
      });
    } on FormatException {
      if (!mounted || generation != _searchGeneration) return;
      setState(() {
        _pageError = context.openchatL10n.modelSearchFailed;
        _failedPageCursor = cursor;
        _failedPageIndex = pageIndex;
        _isSearching = false;
      });
    }
  }

  Future<void> _retrySearch() {
    final failedPageIndex = _failedPageIndex;
    if (failedPageIndex == null) return _searchModels();
    return _loadPage(
      cursor: _failedPageCursor,
      pageIndex: failedPageIndex,
      resetPagination: failedPageIndex == 0,
    );
  }

  Future<void> _loadPreviousPage() {
    if (_isSearching || _pageIndex == 0) return Future<void>.value();
    final pageIndex = _pageIndex - 1;
    return _loadPage(
      cursor: _pageCursors[pageIndex],
      pageIndex: pageIndex,
      resetPagination: false,
    );
  }

  Future<void> _loadNextPage() {
    if (_isSearching || _nextCursor == null) return Future<void>.value();
    final pageIndex = _pageIndex + 1;
    final cursor = pageIndex < _pageCursors.length
        ? _pageCursors[pageIndex]
        : _nextCursor;
    return _loadPage(
      cursor: cursor,
      pageIndex: pageIndex,
      resetPagination: false,
    );
  }

  void _scheduleSearch(String _) {
    _searchDebounce?.cancel();
    _searchDebounce = Timer(
      const Duration(milliseconds: 350),
      () => unawaited(_searchModels()),
    );
  }

  Future<void> _selectModel(HuggingFaceModelSearchResult model) async {
    final repository = _repository;
    if (repository == null) return;
    final generation = ++_detailsGeneration;
    setState(() {
      _selectedModel = model;
      _details = null;
      _selectedGroupId = null;
      _pageError = null;
      _isLoadingDetails = true;
    });
    try {
      final details = await repository.loadDetails(
        repoId: model.repoId,
        format: _format,
      );
      if (!mounted || generation != _detailsGeneration) return;
      setState(() {
        _details = details;
        _selectedGroupId = _firstOrNull(details.downloadGroups)?.id;
        _isLoadingDetails = false;
      });
    } on OpenChatServiceException catch (error) {
      if (!mounted || generation != _detailsGeneration) return;
      setState(() {
        _pageError = _messageForHubError(error);
        _isLoadingDetails = false;
      });
    } on FormatException {
      if (!mounted || generation != _detailsGeneration) return;
      setState(() {
        _pageError = context.openchatL10n.modelSearchFailed;
        _isLoadingDetails = false;
      });
    }
  }

  Future<void> _downloadSelectedGroup() async {
    final model = _selectedModel;
    final groupId = _selectedGroupId;
    final details = _details;
    final controller = widget.downloadController;
    if (model == null ||
        groupId == null ||
        details == null ||
        controller == null) {
      return;
    }
    final group = _firstOrNull(
      details.downloadGroups.where((candidate) => candidate.id == groupId),
    );
    if (group == null ||
        !group.canDownload ||
        details.gated ||
        details.private) {
      return;
    }
    await controller.start(
      repoId: model.repoId,
      revision: details.revision,
      format: _format,
      groupId: groupId,
    );
  }

  Future<void> _downloadComponentFile(HuggingFaceModelFile file) async {
    final model = _selectedModel;
    final details = _details;
    final groupId = _selectedGroupId;
    final controller = widget.downloadController;
    if (model == null ||
        details == null ||
        groupId == null ||
        controller == null) {
      return;
    }
    final group = _firstOrNull(
      details.downloadGroups.where((candidate) => candidate.id == groupId),
    );
    if (file.kind == HuggingFaceModelFileKind.model ||
        file.sizeBytes == null ||
        group?.canDownload != true ||
        details.gated ||
        details.private ||
        controller.isActive) {
      return;
    }

    await controller.start(
      repoId: model.repoId,
      revision: details.revision,
      format: _format,
      groupId: groupId,
      componentPath: file.path,
    );
    final downloadedModel = controller.downloadedModel;
    if (!mounted ||
        controller.status != HuggingFaceDownloadStatus.completed ||
        downloadedModel?.repoId != model.repoId ||
        downloadedModel?.revision != details.revision ||
        downloadedModel?.componentPath != file.path) {
      return;
    }
    setState(
      () => _downloadedComponentKeys.add(_componentKey(model, details, file)),
    );
  }

  String _componentKey(
    HuggingFaceModelSearchResult model,
    HuggingFaceModelDetails details,
    HuggingFaceModelFile file,
  ) =>
      '${_format.wireValue}\n${model.repoId}\n${details.revision}\n${file.path}';

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;

    return LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < 900;
        final headerPadding = constraints.maxWidth < 640 ? 20.0 : 32.0;
        final contentPadding = constraints.maxWidth < 640 ? 16.0 : 28.0;
        final header = Padding(
          padding: EdgeInsets.fromLTRB(headerPadding, 24, headerPadding, 16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                l10n.models,
                style: Theme.of(context).textTheme.headlineSmall,
              ),
              const SizedBox(height: 4),
              Text(l10n.modelsPageDescription),
              const SizedBox(height: 20),
              _formatSelector(palette),
              const SizedBox(height: 14),
              TextField(
                controller: _searchController,
                onChanged: _scheduleSearch,
                onSubmitted: (_) => unawaited(_searchModels()),
                decoration: InputDecoration(
                  hintText: l10n.huggingFaceModelSearchHint,
                  prefixIcon: const Icon(Icons.search_rounded),
                  suffixIcon: _isSearching
                      ? const Padding(
                          padding: EdgeInsets.all(12),
                          child: SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                        )
                      : IconButton(
                          tooltip: l10n.modelSearchRefresh,
                          onPressed: () => unawaited(_searchModels()),
                          icon: const Icon(Icons.refresh_rounded),
                        ),
                ),
              ),
            ],
          ),
        );

        final resultList = _buildResultList(palette);
        final details = _buildDetailsPanel(palette);
        final body = compact
            ? Column(
                children: [
                  SizedBox(height: 240, child: resultList),
                  const SizedBox(height: 12),
                  Expanded(child: details),
                ],
              )
            : Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  SizedBox(width: 340, child: resultList),
                  const SizedBox(width: 16),
                  Expanded(child: details),
                ],
              );

        return ColoredBox(
          color: palette.surface,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              header,
              Expanded(
                child: Padding(
                  padding: EdgeInsets.fromLTRB(
                    contentPadding,
                    0,
                    contentPadding,
                    contentPadding,
                  ),
                  child: body,
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _formatSelector(OpenChatPalette palette) {
    final l10n = context.openchatL10n;
    final options = <(HuggingFaceModelFormat, String)>[
      (HuggingFaceModelFormat.gguf, l10n.modelFormatGguf),
      (HuggingFaceModelFormat.transformers, l10n.modelFormatTransformers),
      (HuggingFaceModelFormat.exllama, l10n.modelFormatExllama),
    ];
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        for (final option in options)
          OutlinedButton(
            onPressed: () {
              if (_format == option.$1) return;
              setState(() => _format = option.$1);
              unawaited(_searchModels());
            },
            style: OutlinedButton.styleFrom(
              minimumSize: const Size(0, 40),
              padding: const EdgeInsets.symmetric(horizontal: 12),
              visualDensity: VisualDensity.standard,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              foregroundColor: _format == option.$1
                  ? palette.text
                  : palette.secondaryText,
              backgroundColor: _format == option.$1
                  ? palette.selected
                  : Colors.transparent,
              side: BorderSide(
                color: _format == option.$1 ? palette.accent : palette.border,
              ),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(OpenChatRadii.control),
              ),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                LocalEngineIcon(
                  engineId: option.$1.engineId,
                  color: _format == option.$1
                      ? palette.accentIcon
                      : palette.secondaryIcon,
                  size: 18,
                ),
                const SizedBox(width: 8),
                Text(option.$2, maxLines: 1, overflow: TextOverflow.ellipsis),
              ],
            ),
          ),
        SizedBox(
          width: 220,
          child: OpenChatSelect<HuggingFaceModelSort>(
            options: <OpenChatSelectOption<HuggingFaceModelSort>>[
              OpenChatSelectOption(
                value: HuggingFaceModelSort.downloads,
                label: l10n.modelSortDownloads,
              ),
              OpenChatSelectOption(
                value: HuggingFaceModelSort.likes,
                label: l10n.modelSortLikes,
              ),
              OpenChatSelectOption(
                value: HuggingFaceModelSort.recentlyUpdated,
                label: l10n.modelSortRecentlyUpdated,
              ),
            ],
            value: _sort,
            onChanged: (value) {
              if (value == _sort) return;
              setState(() => _sort = value);
              unawaited(_searchModels());
            },
            palette: OpenChatPalette.of(context),
            leadingIcon: Icons.sort_rounded,
          ),
        ),
      ],
    );
  }

  Widget _buildResultList(OpenChatPalette palette) {
    final l10n = context.openchatL10n;
    final pageError = _pageError;
    return ChatSurfaceCard(
      child: Column(
        children: [
          Expanded(
            child: pageError != null && _selectedModel == null
                ? _emptyState(
                    pageError,
                    Icons.error_outline_rounded,
                    onRetry: () => unawaited(_retrySearch()),
                  )
                : _isSearching && _models.isEmpty
                ? const Center(child: CircularProgressIndicator())
                : _models.isEmpty
                ? _emptyState(l10n.modelSearchEmpty, Icons.search_off_rounded)
                : ListView.separated(
                    padding: const EdgeInsets.all(8),
                    itemCount: _models.length,
                    separatorBuilder: (_, _) =>
                        Divider(height: 1, color: palette.border),
                    itemBuilder: (context, index) {
                      final model = _models[index];
                      final selected = model.repoId == _selectedModel?.repoId;
                      return Material(
                        key: ValueKey(model.repoId),
                        color: selected ? palette.selected : Colors.transparent,
                        shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(
                            OpenChatRadii.control,
                          ),
                          side: BorderSide(
                            color: selected
                                ? palette.accent
                                : Colors.transparent,
                          ),
                        ),
                        child: InkWell(
                          borderRadius: BorderRadius.circular(
                            OpenChatRadii.control,
                          ),
                          onTap: () => unawaited(_selectModel(model)),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 12,
                              vertical: 11,
                            ),
                            child: Row(
                              children: [
                                _publisherAvatar(model.repoId, palette),
                                const SizedBox(width: 10),
                                Expanded(
                                  child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: [
                                      Text(
                                        model.repoId,
                                        maxLines: 2,
                                        overflow: TextOverflow.ellipsis,
                                        style: TextStyle(
                                          color: palette.text,
                                          fontWeight: selected
                                              ? FontWeight.w600
                                              : FontWeight.w500,
                                        ),
                                      ),
                                      const SizedBox(height: 3),
                                      Text(
                                        '${l10n.modelDownloadsLabel}: ${NumberFormat.compact().format(model.downloads)}',
                                        style: Theme.of(context)
                                            .textTheme
                                            .bodySmall,
                                      ),
                                    ],
                                  ),
                                ),
                                if (model.gated || model.private)
                                  Icon(
                                    Icons.lock_outline_rounded,
                                    size: 17,
                                    color: palette.secondaryIcon,
                                  ),
                                if (selected) ...[
                                  if (model.gated || model.private)
                                    const SizedBox(width: 6),
                                  Icon(
                                    Icons.check_circle_rounded,
                                    size: 18,
                                    color: palette.accent,
                                  ),
                                ],
                              ],
                            ),
                          ),
                        ),
                      );
                    },
                  ),
          ),
          if (_models.isNotEmpty && pageError == null) ...[
            Divider(height: 1, color: palette.border),
            _buildPageControls(),
          ],
        ],
      ),
    );
  }

  Widget _publisherAvatar(String repoId, OpenChatPalette palette) {
    final separator = repoId.indexOf('/');
    final publisher = separator > 0 ? repoId.substring(0, separator) : repoId;
    final avatarUrl = Uri.https(
      'huggingface.co',
      '/api/avatars/$publisher',
    ).toString();

    return Tooltip(
      message: publisher,
      child: CircleAvatar(
        radius: 12,
        backgroundColor: palette.selected,
        child: ClipOval(
          child: Image.network(
            avatarUrl,
            width: 24,
            height: 24,
            fit: BoxFit.cover,
            excludeFromSemantics: true,
            errorBuilder: (context, error, stackTrace) => Center(
              child: Text(
                publisher.substring(0, 1).toUpperCase(),
                style: TextStyle(
                  color: palette.secondaryText,
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildPageControls() {
    final l10n = context.openchatL10n;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          TextButton.icon(
            onPressed: !_isSearching && _pageIndex > 0
                ? () => unawaited(_loadPreviousPage())
                : null,
            icon: const Icon(Icons.chevron_left_rounded, size: 18),
            label: Text(l10n.modelPreviousPage),
          ),
          Text(
            l10n.modelPageLabel(_pageIndex + 1),
            style: Theme.of(context).textTheme.bodySmall,
          ),
          TextButton.icon(
            onPressed: !_isSearching && _nextCursor != null
                ? () => unawaited(_loadNextPage())
                : null,
            iconAlignment: IconAlignment.end,
            icon: const Icon(Icons.chevron_right_rounded, size: 18),
            label: Text(l10n.modelNextPage),
          ),
        ],
      ),
    );
  }

  Widget _buildDetailsPanel(OpenChatPalette palette) {
    final l10n = context.openchatL10n;
    final model = _selectedModel;
    final details = _details;
    if (model == null) {
      return ChatSurfaceCard(
        child: _emptyState(
          l10n.modelChooseForDetails,
          Icons.smart_toy_outlined,
        ),
      );
    }
    if (_isLoadingDetails) {
      return ChatSurfaceCard(
        child: _emptyState(l10n.modelDetailsLoading, null, loading: true),
      );
    }
    if (_pageError != null || details == null) {
      return ChatSurfaceCard(
        child: _emptyState(
          _pageError ?? l10n.modelSearchFailed,
          Icons.error_outline_rounded,
          onRetry: () => unawaited(_selectModel(model)),
        ),
      );
    }

    final selectedGroup = _firstOrNull(
      details.downloadGroups.where((group) => group.id == _selectedGroupId),
    );
    final controller = widget.downloadController;
    final downloadBlocked = details.gated || details.private;
    final canDownload =
        selectedGroup?.canDownload == true &&
        !downloadBlocked &&
        controller != null &&
        controller.isActive == false;
    final canDownloadComponents =
        selectedGroup?.canDownload == true &&
        !downloadBlocked &&
        controller != null &&
        !controller.isActive;
    final formatFolder = switch (_format) {
      HuggingFaceModelFormat.gguf => 'llama',
      HuggingFaceModelFormat.transformers => 'vllm',
      HuggingFaceModelFormat.exllama => 'exllama',
    };
    final componentGroups =
        <(String, HuggingFaceModelFileKind, List<HuggingFaceModelFile>)>[
          (
            l10n.modelVisionComponentsLabel,
            HuggingFaceModelFileKind.vision,
            details.files
                .where((file) => file.kind == HuggingFaceModelFileKind.vision)
                .toList(growable: false),
          ),
          (
            l10n.modelMtpComponentsLabel,
            HuggingFaceModelFileKind.mtp,
            details.files
                .where((file) => file.kind == HuggingFaceModelFileKind.mtp)
                .toList(growable: false),
          ),
          (
            l10n.modelAuxiliaryComponentsLabel,
            HuggingFaceModelFileKind.auxiliary,
            details.files
                .where(
                  (file) => file.kind == HuggingFaceModelFileKind.auxiliary,
                )
                .toList(growable: false),
          ),
        ];
    final modelFiles = details.files
        .where((file) => file.kind == HuggingFaceModelFileKind.model)
        .toList(growable: false);

    return ChatSurfaceCard(
      child: ListView(
        padding: const EdgeInsets.all(20),
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              LocalEngineIcon(
                engineId: _format.engineId,
                color: palette.secondaryIcon,
                size: 24,
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  model.repoId,
                  style: Theme.of(context).textTheme.titleLarge,
                ),
              ),
              if (details.gated || details.private)
                _StatusLabel(
                  text: details.private
                      ? l10n.modelPrivateBadge
                      : l10n.modelGatedBadge,
                  palette: palette,
                ),
            ],
          ),
          const SizedBox(height: 16),
          Wrap(
            spacing: 20,
            runSpacing: 8,
            children: [
              _InfoLabel(
                label: l10n.modelDownloadsLabel,
                value: NumberFormat.compact().format(model.downloads),
              ),
              _InfoLabel(
                label: l10n.modelLikesLabel,
                value: NumberFormat.compact().format(model.likes),
              ),
              if (details.license case final license?)
                _InfoLabel(label: l10n.modelLicenseLabel, value: license),
              _InfoLabel(
                label: l10n.modelRevisionLabel,
                value: details.revision.substring(0, 12),
              ),
            ],
          ),
          const SizedBox(height: 18),
          _buildReadmeSection(model, details, palette),
          const SizedBox(height: 20),
          Text(
            l10n.modelFilesLabel,
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 8),
          if (details.files.isEmpty)
            Text(l10n.modelNoFiles)
          else ...[
            ..._buildModelFileRows(modelFiles, palette),
            for (final group in componentGroups)
              if (group.$3.isNotEmpty) ...[
                Builder(
                  builder: (context) {
                    final groupKey =
                        '${model.repoId}\n${details.revision}\n${group.$2.name}';
                    final visibleCount =
                        _visibleComponentCounts[groupKey] ?? 12;
                    final visibleFiles = group.$3
                        .take(visibleCount)
                        .toList(growable: false);
                    return Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const SizedBox(height: 14),
                        Text(
                          group.$1,
                          style: Theme.of(context).textTheme.titleSmall,
                        ),
                        const SizedBox(height: 6),
                        ..._buildModelFileRows(
                          visibleFiles,
                          palette,
                          canDownloadComponents: canDownloadComponents,
                        ),
                        if (group.$3.length > visibleFiles.length)
                          TextButton.icon(
                            onPressed: () => setState(() {
                              _visibleComponentCounts[groupKey] =
                                  (visibleCount + 12)
                                      .clamp(0, group.$3.length)
                                      .toInt();
                            }),
                            icon: const Icon(
                              Icons.expand_more_rounded,
                              size: 18,
                            ),
                            label: Text(l10n.modelShowMoreComponents),
                          ),
                      ],
                    );
                  },
                ),
              ],
          ],
          const SizedBox(height: 18),
          Text(
            l10n.modelDownloadOptionsLabel,
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 8),
          if (details.downloadGroups.isEmpty)
            Text(l10n.modelNoCompatibleFiles)
          else
            _DownloadOptionsTable(
              groups: details.downloadGroups,
              selectedGroupId: _selectedGroupId,
              onSelected: (groupId) =>
                  setState(() => _selectedGroupId = groupId),
              palette: palette,
              groupLabel: l10n.modelDownloadGroupLabel,
              sizeLabel: l10n.modelDownloadSizeLabel,
            ),
          const SizedBox(height: 12),
          Text(l10n.modelSavedToFolder(formatFolder)),
          if (downloadBlocked) ...[
            const SizedBox(height: 8),
            _InlineNotice(
              text: l10n.modelDownloadAccessNeeded,
              icon: Icons.lock_outline_rounded,
              palette: palette,
            ),
          ] else if (selectedGroup?.canDownload == false) ...[
            const SizedBox(height: 8),
            _InlineNotice(
              text: l10n.modelUnknownDownloadSize,
              icon: Icons.info_outline_rounded,
              palette: palette,
            ),
          ] else if (selectedGroup == null) ...[
            const SizedBox(height: 8),
            _InlineNotice(
              text: l10n.modelNoCompatibleFiles,
              icon: Icons.info_outline_rounded,
              palette: palette,
            ),
          ],
          if (controller?.isActive == true) ...[
            const SizedBox(height: 16),
            _DownloadProgress(controller: controller!, palette: palette),
            const SizedBox(height: 10),
            Align(
              alignment: Alignment.centerRight,
              child: TextButton.icon(
                onPressed:
                    controller.status == HuggingFaceDownloadStatus.cancelling
                    ? null
                    : () => unawaited(controller.cancel()),
                icon: const Icon(Icons.close_rounded, size: 17),
                label: Text(l10n.modelCancelDownload),
              ),
            ),
          ] else if (controller?.status ==
                  HuggingFaceDownloadStatus.completed &&
              controller?.downloadedModel?.repoId == model.repoId &&
              controller?.downloadedModel?.revision == details.revision &&
              controller?.downloadedModel?.componentPath == null) ...[
            const SizedBox(height: 12),
            _InlineNotice(
              text: l10n.modelDownloadComplete,
              icon: Icons.check_circle_outline_rounded,
              palette: palette,
            ),
          ] else if (controller?.status ==
                  HuggingFaceDownloadStatus.cancelled &&
              controller?.activeRepoId == model.repoId) ...[
            const SizedBox(height: 12),
            _InlineNotice(
              text: l10n.modelDownloadCancelled,
              icon: Icons.info_outline_rounded,
              palette: palette,
            ),
          ] else if (controller?.status == HuggingFaceDownloadStatus.failed &&
              controller?.activeRepoId == model.repoId) ...[
            const SizedBox(height: 12),
            _InlineNotice(
              text: switch (controller?.error?.code) {
                'hugging_face_access_denied' => l10n.modelDownloadAccessNeeded,
                'hugging_face_revision_changed' => l10n.modelRevisionChanged,
                _ => l10n.modelDownloadFailed,
              },
              icon: Icons.error_outline_rounded,
              palette: palette,
            ),
          ],
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              onPressed: canDownload
                  ? () => unawaited(_downloadSelectedGroup())
                  : null,
              icon: const Icon(Icons.download_rounded),
              label: Text(l10n.modelDownloadButton),
            ),
          ),
        ],
      ),
    );
  }

  List<Widget> _buildModelFileRows(
    List<HuggingFaceModelFile> files,
    OpenChatPalette palette, {
    bool canDownloadComponents = false,
  }) {
    final l10n = context.openchatL10n;
    final model = _selectedModel;
    final details = _details;
    return [
      for (final file in files.take(12))
        Builder(
          builder: (context) {
            final isComponent = file.kind != HuggingFaceModelFileKind.model;
            final lastDownloadedComponent =
                widget.downloadController?.downloadedModel;
            final isDownloaded =
                model != null &&
                details != null &&
                (_downloadedComponentKeys.contains(
                      _componentKey(model, details, file),
                    ) ||
                    (lastDownloadedComponent?.repoId == model.repoId &&
                        lastDownloadedComponent?.revision == details.revision &&
                        lastDownloadedComponent?.componentPath == file.path));
            final canDownload =
                isComponent &&
                file.sizeBytes != null &&
                canDownloadComponents &&
                !isDownloaded;
            return Padding(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Row(
                children: [
                  Icon(
                    Icons.insert_drive_file_outlined,
                    size: 16,
                    color: palette.secondaryIcon,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      file.path,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  const SizedBox(width: 10),
                  Text(
                    _formatBytes(file.sizeBytes),
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  if (isComponent) ...[
                    const SizedBox(width: 4),
                    IconButton(
                      tooltip: isDownloaded
                          ? l10n.modelComponentDownloaded
                          : l10n.modelDownloadComponentButton,
                      onPressed: canDownload
                          ? () => unawaited(_downloadComponentFile(file))
                          : null,
                      visualDensity: VisualDensity.compact,
                      constraints: const BoxConstraints.tightFor(
                        width: 36,
                        height: 36,
                      ),
                      icon: Icon(
                        isDownloaded
                            ? Icons.check_circle_outline_rounded
                            : Icons.download_rounded,
                        size: 18,
                      ),
                    ),
                  ],
                ],
              ),
            );
          },
        ),
      if (files.length > 12)
        Padding(
          padding: const EdgeInsets.only(top: 4),
          child: Text(
            context.openchatL10n.toolMoreFilesAvailable,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ),
    ];
  }

  Widget _buildReadmeSection(
    HuggingFaceModelSearchResult model,
    HuggingFaceModelDetails details,
    OpenChatPalette palette,
  ) {
    final l10n = context.openchatL10n;
    final status = details.readmeStatus;
    final readmeMarkdown = details.readmeMarkdown;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          l10n.modelReadmeLabel,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 8),
        if (status == HuggingFaceModelReadmeStatus.available &&
            readmeMarkdown != null)
          _ModelReadmeContent(markdown: readmeMarkdown, palette: palette)
        else if (status == HuggingFaceModelReadmeStatus.unavailable) ...[
          Text(_messageForReadmeError(details.readmeErrorCode)),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              onPressed: () => unawaited(_selectModel(model)),
              icon: const Icon(Icons.refresh_rounded, size: 17),
              label: Text(l10n.retry),
            ),
          ),
        ] else
          Text(switch (status) {
            HuggingFaceModelReadmeStatus.missing => l10n.modelReadmeMissing,
            HuggingFaceModelReadmeStatus.accessDenied =>
              l10n.modelReadmeAccessDenied,
            HuggingFaceModelReadmeStatus.tooLarge => l10n.modelReadmeTooLarge,
            HuggingFaceModelReadmeStatus.available =>
              l10n.modelReadmeUnavailable,
            HuggingFaceModelReadmeStatus.unavailable =>
              l10n.modelReadmeUnavailable,
          }),
      ],
    );
  }

  String _messageForReadmeError(String? code) {
    final l10n = context.openchatL10n;
    return switch (code) {
      'hugging_face_rate_limited' => l10n.modelSearchRateLimited,
      'service_timeout' => l10n.modelSearchTimedOut,
      _ => l10n.modelReadmeUnavailable,
    };
  }

  String _messageForHubError(OpenChatServiceException error) {
    final l10n = context.openchatL10n;
    return switch (error.code) {
      'hugging_face_rate_limited' => l10n.modelSearchRateLimited,
      'hugging_face_unavailable' => l10n.modelSearchUnavailable,
      'hugging_face_response_invalid' => l10n.modelSearchInvalidResponse,
      'hugging_face_access_denied' => l10n.modelDownloadAccessNeeded,
      'service_timeout' => l10n.modelSearchTimedOut,
      _ => l10n.modelSearchFailed,
    };
  }

  Widget _emptyState(
    String message,
    IconData? icon, {
    bool loading = false,
    VoidCallback? onRetry,
  }) {
    final palette = OpenChatPalette.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (loading)
              const CircularProgressIndicator()
            else if (icon != null)
              Icon(icon, color: palette.secondaryIcon, size: 28),
            const SizedBox(height: 12),
            Text(message, textAlign: TextAlign.center),
            if (onRetry != null) ...[
              const SizedBox(height: 12),
              OutlinedButton.icon(
                onPressed: onRetry,
                icon: const Icon(Icons.refresh_rounded),
                label: Text(context.openchatL10n.retry),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _ModelReadmeContent extends StatefulWidget {
  const _ModelReadmeContent({required this.markdown, required this.palette});

  final String markdown;
  final OpenChatPalette palette;

  @override
  State<_ModelReadmeContent> createState() => _ModelReadmeContentState();
}

class _ModelReadmeContentState extends State<_ModelReadmeContent> {
  String? _source;
  String? _safeHtml;

  @override
  void didUpdateWidget(covariant _ModelReadmeContent oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.markdown != widget.markdown) {
      _source = null;
      _safeHtml = null;
    }
  }

  @override
  Widget build(BuildContext context) {
    final source = widget.markdown;
    final cachedHtml = _safeHtml;
    final safeHtml = _source == source && cachedHtml != null
        ? cachedHtml
        : markdownToSafeHtml(source);
    _source = source;
    _safeHtml = safeHtml;
    final conversationStyle = OpenChatConversationStyle.of(context);
    final textStyle =
        Theme.of(context).textTheme.bodyMedium?.copyWith(
          color: widget.palette.text,
          fontFamily: conversationStyle.fontFamily,
          height: 1.55,
        ) ??
        TextStyle(
          color: widget.palette.text,
          fontFamily: conversationStyle.fontFamily,
          height: 1.55,
        );
    return HtmlWidget(
      safeHtml,
      enableCaching: true,
      renderMode: RenderMode.column,
      textStyle: textStyle,
      onTapUrl: (_) => true,
      customStylesBuilder: (element) => switch (element.localName) {
        'pre' => {
          'background-color': _cssColor(widget.palette.composer),
          'padding': '12px',
          'white-space': 'pre-wrap',
        },
        'code' => {
          'background-color': _cssColor(widget.palette.composer),
          'font-family': 'monospace',
        },
        'blockquote' => {
          'border-left': '2px solid ${_cssColor(widget.palette.border)}',
          'padding-left': '12px',
          'color': _cssColor(widget.palette.secondaryText),
        },
        'th' || 'td' => {
          'border': '1px solid ${_cssColor(widget.palette.border)}',
          'padding': '6px',
        },
        _ => null,
      },
    );
  }
}

class _DownloadOptionsTable extends StatelessWidget {
  const _DownloadOptionsTable({
    required this.groups,
    required this.selectedGroupId,
    required this.onSelected,
    required this.palette,
    required this.groupLabel,
    required this.sizeLabel,
  });

  final List<HuggingFaceDownloadGroup> groups;
  final String? selectedGroupId;
  final ValueChanged<String> onSelected;
  final OpenChatPalette palette;
  final String groupLabel;
  final String sizeLabel;

  @override
  Widget build(BuildContext context) {
    return RadioGroup<String>(
      groupValue: selectedGroupId,
      onChanged: (value) {
        if (value != null) onSelected(value);
      },
      child: Table(
        columnWidths: const <int, TableColumnWidth>{
          0: FixedColumnWidth(44),
          1: FlexColumnWidth(5),
          2: FlexColumnWidth(1.2),
        },
        defaultVerticalAlignment: TableCellVerticalAlignment.middle,
        children: [
          TableRow(
            decoration: BoxDecoration(
              border: Border(bottom: BorderSide(color: palette.border)),
            ),
            children: [
              const SizedBox(height: 38),
              _headerCell(context, groupLabel),
              _headerCell(context, sizeLabel, alignEnd: true),
            ],
          ),
          for (final group in groups)
            _groupRow(context, group, group.id == selectedGroupId),
        ],
      ),
    );
  }

  Widget _headerCell(
    BuildContext context,
    String label, {
    bool alignEnd = false,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 10),
      child: Text(
        label,
        textAlign: alignEnd ? TextAlign.end : TextAlign.start,
        style: Theme.of(context).textTheme.bodySmall?.copyWith(
          color: palette.secondaryText,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }

  TableRow _groupRow(
    BuildContext context,
    HuggingFaceDownloadGroup group,
    bool selected,
  ) {
    final enabled = group.canDownload;
    return TableRow(
      decoration: BoxDecoration(
        color: selected ? palette.selected : Colors.transparent,
        border: Border(bottom: BorderSide(color: palette.border)),
      ),
      children: [
        _groupCell(
          group,
          enabled,
          Center(
            child: Radio<String>(value: group.id, enabled: enabled),
          ),
        ),
        _groupCell(
          group,
          enabled,
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              group.displayName,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ),
        _groupCell(
          group,
          enabled,
          Align(
            alignment: Alignment.centerRight,
            child: Text(
              _formatBytes(group.totalBytes),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
        ),
      ],
    );
  }

  Widget _groupCell(
    HuggingFaceDownloadGroup group,
    bool enabled,
    Widget child,
  ) {
    return InkWell(
      onTap: enabled ? () => onSelected(group.id) : null,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 9),
        child: child,
      ),
    );
  }
}

class _DownloadProgress extends StatelessWidget {
  const _DownloadProgress({required this.controller, required this.palette});

  final HuggingFaceDownloadController controller;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final progress = controller.progress;
    final downloadingModel = controller.activeRepoId ?? '';
    final fraction = progress?.fraction;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          progress == null
              ? downloadingModel
              : l10n.modelDownloadRunning(
                  progress.fileName,
                  progress.fileIndex,
                  progress.fileCount,
                ),
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          style: Theme.of(context).textTheme.bodyMedium
              ?.copyWith(color: palette.text),
        ),
        const SizedBox(height: 8),
        LinearProgressIndicator(value: fraction),
        if (controller.progressUnavailable) ...[
          const SizedBox(height: 5),
          Text(
            l10n.modelDownloadProgressUnavailable,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
        if (progress != null && progress.totalBytes > 0) ...[
          const SizedBox(height: 5),
          Text(
            '${_formatBytes(progress.downloadedBytes)} / ${_formatBytes(progress.totalBytes)}',
            textAlign: TextAlign.end,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ],
    );
  }
}

class _InfoLabel extends StatelessWidget {
  const _InfoLabel({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    return Text.rich(
      TextSpan(
        children: [
          TextSpan(
            text: '$label: ',
            style: TextStyle(color: palette.secondaryText),
          ),
          TextSpan(
            text: value,
            style: TextStyle(color: palette.text),
          ),
        ],
      ),
    );
  }
}

class _StatusLabel extends StatelessWidget {
  const _StatusLabel({required this.text, required this.palette});

  final String text;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: palette.disabledSurface,
        borderRadius: BorderRadius.circular(OpenChatRadii.control),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
        child: Text(text, style: Theme.of(context).textTheme.bodySmall),
      ),
    );
  }
}

class _InlineNotice extends StatelessWidget {
  const _InlineNotice({
    required this.text,
    required this.icon,
    required this.palette,
  });

  final String text;
  final IconData icon;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(icon, size: 17, color: palette.secondaryIcon),
        const SizedBox(width: 8),
        Expanded(
          child: Text(text, style: Theme.of(context).textTheme.bodySmall),
        ),
      ],
    );
  }
}

String _formatBytes(int? bytes) {
  if (bytes == null) return '—';
  if (bytes < 1000) return '$bytes B';
  const units = <String>['KB', 'MB', 'GB', 'TB'];
  var amount = bytes.toDouble();
  var unitIndex = -1;
  do {
    amount /= 1000;
    unitIndex++;
  } while (amount >= 1000 && unitIndex < units.length - 1);
  return '${amount.toStringAsFixed(1)} ${units[unitIndex]}';
}

String _cssColor(Color color) {
  final rgb = color.toARGB32() & 0x00ffffff;
  return '#${rgb.toRadixString(16).padLeft(6, '0')}';
}

T? _firstOrNull<T>(Iterable<T> values) {
  final iterator = values.iterator;
  return iterator.moveNext() ? iterator.current : null;
}
