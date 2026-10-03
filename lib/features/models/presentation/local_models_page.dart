import 'dart:async';

import 'package:flutter/material.dart';

import 'package:openchat/app/local_engine_icon.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_surface_card.dart';
import 'package:openchat/features/settings/data/local_engines_models.dart';
import 'package:openchat/features/settings/data/local_engines_repository.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

enum _LocalModelsLoadState { loading, loaded, unavailable, failed }

class LocalModelsPage extends StatefulWidget {
  const LocalModelsPage({
    required this.serviceClient,
    required this.onOpenModelCatalog,
    super.key,
  });

  final OpenChatServiceClient? serviceClient;
  final VoidCallback onOpenModelCatalog;

  @override
  State<LocalModelsPage> createState() => _LocalModelsPageState();
}

class _LocalModelsPageState extends State<LocalModelsPage> {
  _LocalModelsLoadState _pageState = _LocalModelsLoadState.loading;
  List<LocalRegisteredModel> _models = const <LocalRegisteredModel>[];
  int _loadGeneration = 0;

  @override
  void initState() {
    super.initState();
    unawaited(_loadModels());
  }

  @override
  void didUpdateWidget(covariant LocalModelsPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.serviceClient != widget.serviceClient) {
      unawaited(_loadModels());
    }
  }

  Future<void> _loadModels() async {
    final generation = ++_loadGeneration;
    final service = widget.serviceClient;
    if (service == null) {
      setState(() {
        _models = const <LocalRegisteredModel>[];
        _pageState = _LocalModelsLoadState.unavailable;
      });
      return;
    }

    setState(() => _pageState = _LocalModelsLoadState.loading);
    try {
      final catalog = await LocalEnginesRepository(service).loadModels();
      if (!mounted || generation != _loadGeneration) return;
      final models = catalog.models.toList()
        ..sort((left, right) {
          final engineOrder = left.engineId.compareTo(right.engineId);
          return engineOrder != 0
              ? engineOrder
              : left.displayName.toLowerCase().compareTo(
                  right.displayName.toLowerCase(),
                );
        });
      setState(() {
        _models = models;
        _pageState = _LocalModelsLoadState.loaded;
      });
    } on OpenChatServiceException {
      if (!mounted || generation != _loadGeneration) return;
      setState(() {
        _models = const <LocalRegisteredModel>[];
        _pageState = _LocalModelsLoadState.failed;
      });
    } on FormatException {
      if (!mounted || generation != _loadGeneration) return;
      setState(() {
        _models = const <LocalRegisteredModel>[];
        _pageState = _LocalModelsLoadState.failed;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final compact = MediaQuery.sizeOf(context).width < 640;
    final heading = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          l10n.localModelsPageTitle,
          style: Theme.of(context).textTheme.headlineSmall,
        ),
        const SizedBox(height: 6),
        Text(
          l10n.localModelsPageDescription,
          style: Theme.of(context).textTheme.bodyMedium
              ?.copyWith(color: palette.secondaryText),
        ),
      ],
    );
    final reloadButton = OutlinedButton.icon(
      onPressed: _pageState == _LocalModelsLoadState.loading
          ? null
          : () => unawaited(_loadModels()),
      icon: const Icon(Icons.refresh_rounded),
      label: Text(l10n.localModelsRefresh),
    );

    return SingleChildScrollView(
      padding: EdgeInsets.fromLTRB(
        compact ? 18 : 32,
        24,
        compact ? 18 : 32,
        32,
      ),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1120),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (compact)
                Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    heading,
                    const SizedBox(height: 12),
                    Align(
                      alignment: Alignment.centerRight,
                      child: reloadButton,
                    ),
                  ],
                )
              else
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Expanded(child: heading),
                    const SizedBox(width: 12),
                    reloadButton,
                  ],
                ),
              const SizedBox(height: 20),
              ChatSurfaceCard(
                child: switch (_pageState) {
                  _LocalModelsLoadState.loading => const Padding(
                    padding: EdgeInsets.all(32),
                    child: Center(child: CircularProgressIndicator()),
                  ),
                  _LocalModelsLoadState.unavailable => _MessageState(
                    icon: Icons.cloud_off_outlined,
                    message: l10n.localEnginesUnavailable,
                    palette: palette,
                  ),
                  _LocalModelsLoadState.failed => _MessageState(
                    icon: Icons.error_outline_rounded,
                    message: l10n.localModelsLoadFailed,
                    palette: palette,
                    actionLabel: l10n.localModelsRefresh,
                    actionIcon: Icons.refresh_rounded,
                    onAction: () => unawaited(_loadModels()),
                  ),
                  _LocalModelsLoadState.loaded when _models.isEmpty =>
                    _MessageState(
                      icon: Icons.folder_open_outlined,
                      message: l10n.localModelsPageEmpty,
                      palette: palette,
                      actionLabel: l10n.localModelsDiscover,
                      onAction: widget.onOpenModelCatalog,
                    ),
                  _LocalModelsLoadState.loaded => _ModelList(
                    models: _models,
                    palette: palette,
                  ),
                },
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ModelList extends StatelessWidget {
  const _ModelList({required this.models, required this.palette});

  final List<LocalRegisteredModel> models;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        for (var index = 0; index < models.length; index++) ...[
          if (index > 0) Divider(height: 1, color: palette.border),
          _ModelListTile(model: models[index], palette: palette),
        ],
      ],
    );
  }
}

class _ModelListTile extends StatelessWidget {
  const _ModelListTile({required this.model, required this.palette});

  final LocalRegisteredModel model;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final status = model.isAvailable
        ? l10n.localEngineAvailable
        : !model.pathExists
        ? l10n.localModelPathMissing
        : l10n.localModelEngineNotReady;

    return LayoutBuilder(
      builder: (context, constraints) {
        final narrow = constraints.maxWidth < 640;
        final icon = Padding(
          padding: const EdgeInsets.only(top: 2),
          child: LocalEngineIcon(
            engineId: model.engineId,
            color: palette.secondaryIcon,
            size: 22,
          ),
        );
        final information = Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              model.displayName,
              style: Theme.of(context).textTheme.titleSmall,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            const SizedBox(height: 4),
            Text(
              _engineName(model.engineId),
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
            ),
            const SizedBox(height: 2),
            Text(
              model.path,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
          ],
        );
        final availability = Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              model.isAvailable
                  ? Icons.check_circle_outline_rounded
                  : Icons.info_outline_rounded,
              size: 16,
              color: model.isAvailable
                  ? palette.accentIcon
                  : palette.secondaryIcon,
            ),
            const SizedBox(width: 6),
            Flexible(
              child: Text(
                status,
                style: Theme.of(context).textTheme.bodySmall
                    ?.copyWith(color: palette.secondaryText),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        );

        return Padding(
          padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
          child: narrow
              ? Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        icon,
                        const SizedBox(width: 14),
                        Expanded(child: information),
                      ],
                    ),
                    Padding(
                      padding: const EdgeInsets.only(left: 36, top: 8),
                      child: availability,
                    ),
                  ],
                )
              : Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    icon,
                    const SizedBox(width: 14),
                    Expanded(child: information),
                    const SizedBox(width: 16),
                    ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 240),
                      child: availability,
                    ),
                  ],
                ),
        );
      },
    );
  }

  String _engineName(String engineId) => switch (engineId) {
    'llama_cpp' => 'llama.cpp',
    'vllm' => 'vLLM',
    'exllama' => 'ExLlamaV3',
    _ => engineId,
  };
}

class _MessageState extends StatelessWidget {
  const _MessageState({
    required this.icon,
    required this.message,
    required this.palette,
    this.actionLabel,
    this.actionIcon = Icons.view_list_rounded,
    this.onAction,
  });

  final IconData icon;
  final String message;
  final OpenChatPalette palette;
  final String? actionLabel;
  final IconData actionIcon;
  final VoidCallback? onAction;

  @override
  Widget build(BuildContext context) {
    final action = onAction;
    final actionText = actionLabel;
    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        children: [
          Icon(icon, color: palette.secondaryIcon, size: 24),
          const SizedBox(height: 10),
          Text(message, textAlign: TextAlign.center),
          if (action != null && actionText != null) ...[
            const SizedBox(height: 14),
            OutlinedButton.icon(
              onPressed: action,
              icon: Icon(actionIcon),
              label: Text(actionText),
            ),
          ],
        ],
      ),
    );
  }
}
