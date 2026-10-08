import 'dart:async';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/local_engine_icon.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/settings/data/local_engines_models.dart';
import 'package:openchat/features/settings/data/local_engines_repository.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/presentation/local_model_directory_controls.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

enum _LocalEnginesLoadState { loading, loaded, unavailable, failed }

class LocalEnginesSettingsSection extends StatefulWidget {
  const LocalEnginesSettingsSection({
    required this.serviceClient,
    required this.settingsPreferences,
    super.key,
  });

  final OpenChatServiceClient? serviceClient;
  final SettingsPreferences settingsPreferences;

  @override
  State<LocalEnginesSettingsSection> createState() =>
      _LocalEnginesSettingsSectionState();
}

class _LocalEnginesSettingsSectionState
    extends State<LocalEnginesSettingsSection> {
  _LocalEnginesLoadState _loadState = _LocalEnginesLoadState.loading;
  LocalEngineCatalog? _catalog;
  String? _installError;
  String? _installingEngineId;
  String? _installingVariantId;
  LocalEngineInstallOperation? _activeOperation;
  LocalEngineInstallProgress? _latestProgress;
  final Map<String, LocalEngineInstallProgress> _assetProgress = {};
  StreamSubscription<LocalEngineInstallProgress>? _progressSubscription;
  bool _isCancelling = false;
  bool _progressStreamFailed = false;
  bool _cancelRequested = false;
  String? _modelActionError;
  String? _registeringEngineId;
  String? _scanningEngineId;
  final Map<String, String> _modelDirectories = {};
  final Set<String> _customModelDirectoryIds = {};
  String? _startingModelId;
  String? _stoppingModelId;
  String? _removingModelId;
  LocalEngineRuntimeOperation? _runtimeOperation;
  bool _isCancellingRuntime = false;
  bool _isSavingExecutablePath = false;
  bool _isDetectingExternalServer = false;
  bool _isConnectingExternalServer = false;
  bool _isDisconnectingExternalServer = false;
  bool _initialExternalServerScanStarted = false;
  final Set<String> _handledExternalServerCandidates = <String>{};
  int _loadGeneration = 0;

  @override
  void initState() {
    super.initState();
    unawaited(_loadCatalog());
  }

  @override
  void didUpdateWidget(covariant LocalEnginesSettingsSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.serviceClient != widget.serviceClient ||
        oldWidget.settingsPreferences != widget.settingsPreferences) {
      unawaited(_loadCatalog());
    }
  }

  @override
  void dispose() {
    unawaited(_progressSubscription?.cancel());
    super.dispose();
  }

  Future<void> _loadCatalog() async {
    if (!mounted) return;
    final service = widget.serviceClient;
    final generation = ++_loadGeneration;
    if (service == null) {
      if (mounted) {
        setState(() {
          _catalog = null;
          _loadState = _LocalEnginesLoadState.unavailable;
        });
      }
      return;
    }

    setState(() {
      _loadState = _LocalEnginesLoadState.loading;
    });

    try {
      final catalog = await LocalEnginesRepository(service).loadCatalog();
      final modelDirectories = <String, String>{};
      final customDirectoryIds = <String>{};
      for (final engine in catalog.engines) {
        final customDirectory = await widget.settingsPreferences
            .readLocalModelDirectory(engine.engineId);
        if (customDirectory == null) {
          modelDirectories[engine.engineId] = engine.modelDirectory;
        } else {
          modelDirectories[engine.engineId] = customDirectory;
          customDirectoryIds.add(engine.engineId);
        }
      }
      if (!mounted || generation != _loadGeneration) return;
      setState(() {
        _catalog = catalog;
        _modelDirectories
          ..clear()
          ..addAll(modelDirectories);
        _customModelDirectoryIds
          ..clear()
          ..addAll(customDirectoryIds);
        _loadState = _LocalEnginesLoadState.loaded;
      });
      if (!_initialExternalServerScanStarted &&
          defaultTargetPlatform == TargetPlatform.windows) {
        _initialExternalServerScanStarted = true;
        await _detectExternalLlamaServers();
      }
    } on OpenChatServiceException {
      if (!mounted || generation != _loadGeneration) return;
      setState(() {
        _catalog = null;
        _loadState = _LocalEnginesLoadState.failed;
      });
    } on FormatException {
      if (!mounted || generation != _loadGeneration) return;
      setState(() {
        _catalog = null;
        _loadState = _LocalEnginesLoadState.failed;
      });
    } on PlatformException {
      if (!mounted || generation != _loadGeneration) return;
      setState(() {
        _catalog = null;
        _loadState = _LocalEnginesLoadState.failed;
      });
    }
  }

  Future<void> _detectExternalLlamaServers({bool allowRepeat = false}) async {
    final service = widget.serviceClient;
    if (service == null || _isDetectingExternalServer) return;
    setState(() {
      _isDetectingExternalServer = true;
      _modelActionError = null;
    });

    List<LocalExternalLlamaServerCandidate> servers;
    try {
      servers = await LocalEnginesRepository(service)
          .detectExternalLlamaServers();
    } on OpenChatServiceException catch (error) {
      if (mounted) setState(() => _modelActionError = error.code);
      return;
    } on FormatException {
      if (mounted) setState(() => _modelActionError = 'invalid_response');
      return;
    } on PlatformException {
      if (mounted) setState(() => _modelActionError = 'scan_failed');
      return;
    } finally {
      if (mounted) setState(() => _isDetectingExternalServer = false);
    }

    if (!mounted) return;
    final connected = _catalog?.externalLlamaServers ?? const [];
    if (connected.any(
      (connectedServer) =>
          !servers.any((server) => server.key == connectedServer.key),
    )) {
      await _loadCatalog();
      if (!mounted) return;
    }
    if (servers.isEmpty) {
      if (allowRepeat) {
        showOpenChatToast(
          context,
          context.openchatL10n.localEngineExternalServerNotFound,
          type: OpenChatToastType.info,
        );
      }
      return;
    }

    for (final server in servers) {
      if (!mounted) return;
      final connected = _catalog?.externalLlamaServers.any(
        (connectedServer) => connectedServer.key == server.key,
      );
      if (connected == true) {
        continue;
      }
      if (!allowRepeat && !_handledExternalServerCandidates.add(server.key)) {
        continue;
      }
      _handledExternalServerCandidates.add(server.key);

      final shouldConnect = await showDialog<bool>(
        context: context,
        builder: (dialogContext) {
          final l10n = dialogContext.openchatL10n;
          return AlertDialog(
            title: Text(l10n.localEngineExternalServerFoundTitle),
            content: SizedBox(
              width: 420,
              child: Text(
                l10n.localEngineExternalServerFoundDescription(server.port),
              ),
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(dialogContext).pop(false),
                child: Text(l10n.localEngineExternalServerNotNow),
              ),
              FilledButton.icon(
                onPressed: () => Navigator.of(dialogContext).pop(true),
                icon: const Icon(LucideIcons.link, size: 16),
                label: Text(l10n.localEngineExternalServerConnect),
              ),
            ],
          );
        },
      );
      if (shouldConnect != true || !mounted) continue;

      setState(() => _isConnectingExternalServer = true);
      try {
        final state = await LocalEnginesRepository(service)
            .connectExternalLlamaServer(server);
        await _loadCatalog();
        if (mounted) {
          showOpenChatToast(
            context,
            context.openchatL10n.localEngineExternalServerConnected(
              server.port,
              state.modelIds.length,
            ),
            type: OpenChatToastType.success,
          );
        }
      } on OpenChatServiceException catch (error) {
        if (mounted) setState(() => _modelActionError = error.code);
      } on FormatException {
        if (mounted) setState(() => _modelActionError = 'invalid_response');
      } on PlatformException {
        if (mounted) setState(() => _modelActionError = 'connect_failed');
      } finally {
        if (mounted) setState(() => _isConnectingExternalServer = false);
      }
    }
  }

  Future<void> _disconnectExternalLlamaServer(
    LocalExternalLlamaServerState server,
  ) async {
    final service = widget.serviceClient;
    if (service == null || _isDisconnectingExternalServer) return;
    setState(() {
      _isDisconnectingExternalServer = true;
      _modelActionError = null;
    });
    try {
      await LocalEnginesRepository(service)
          .disconnectExternalLlamaServer(server);
      await _loadCatalog();
    } on OpenChatServiceException catch (error) {
      if (mounted) setState(() => _modelActionError = error.code);
    } on FormatException {
      if (mounted) setState(() => _modelActionError = 'invalid_response');
    } on PlatformException {
      if (mounted) setState(() => _modelActionError = 'connect_failed');
    } finally {
      if (mounted) setState(() => _isDisconnectingExternalServer = false);
    }
  }

  Future<void> _install(LocalEngine engine, LocalEngineVariant variant) async {
    final service = widget.serviceClient;
    if (service == null || !variant.canInstall || _activeOperation != null) {
      return;
    }

    _cancelRequested = false;
    _progressStreamFailed = false;
    setState(() {
      _installError = null;
      _installingEngineId = engine.engineId;
      _installingVariantId = variant.variantId;
      _latestProgress = null;
      _assetProgress.clear();
      _isCancelling = false;
    });

    LocalEngineInstallOperation? operation;
    Object? failure;
    try {
      operation = await LocalEnginesRepository(service)
          .install(engineId: engine.engineId, variantId: variant.variantId);
      if (!mounted) {
        await operation.cancel();
        return;
      }

      setState(() => _activeOperation = operation);
      _progressSubscription = operation.progress.listen(
        _handleProgress,
        onError: (Object error, StackTrace stackTrace) {
          _progressStreamFailed = true;
        },
      );
      final result = await operation.result;
      if (_progressStreamFailed) {
        throw const FormatException('The local engine progress was invalid.');
      }
      if (!result.installed) {
        throw const FormatException(
          'The local engine service did not confirm installation.',
        );
      }
    } catch (error) {
      failure = error;
    } finally {
      await _progressSubscription?.cancel();
      _progressSubscription = null;
      final cancelled = _cancelRequested;
      if (mounted) {
        setState(() {
          _activeOperation = null;
          _installingEngineId = null;
          _installingVariantId = null;
          _latestProgress = null;
          _assetProgress.clear();
          _isCancelling = false;
          _installError = failure == null || cancelled ? null : 'install';
        });
      }
      await _loadCatalog();
    }
  }

  void _handleProgress(LocalEngineInstallProgress progress) {
    if (!mounted || progress.engineId != _installingEngineId) return;
    if (progress.variantId != _installingVariantId) return;
    setState(() {
      _latestProgress = progress;
      _assetProgress[progress.assetName] = progress;
    });
  }

  Future<void> _cancelInstall() async {
    final operation = _activeOperation;
    if (operation == null || _isCancelling) return;

    _cancelRequested = true;
    setState(() => _isCancelling = true);
    final cancelled = await operation.cancel();
    if (!cancelled && mounted) {
      _cancelRequested = false;
      setState(() => _isCancelling = false);
    }
  }

  String _modelDirectoryFor(LocalEngine engine) =>
      _modelDirectories[engine.engineId] ?? engine.modelDirectory;

  Future<void> _chooseModelDirectory(LocalEngine engine) async {
    if (_registeringEngineId != null || _scanningEngineId != null) return;
    String? path;
    try {
      path = await FilePicker.getDirectoryPath(
        dialogTitle: context.openchatL10n.localModelChooseDirectory,
      );
    } on PlatformException {
      if (mounted) setState(() => _modelActionError = 'picker');
      return;
    }
    final selectedPath = path;
    if (selectedPath == null || selectedPath.trim().isEmpty || !mounted) return;

    try {
      await widget.settingsPreferences.writeLocalModelDirectory(
        engine.engineId,
        selectedPath,
      );
    } on PlatformException {
      if (mounted) setState(() => _modelActionError = 'preferences');
      return;
    } on ArgumentError {
      if (mounted) setState(() => _modelActionError = 'preferences');
      return;
    }

    setState(() {
      _modelActionError = null;
      _modelDirectories[engine.engineId] = selectedPath;
      _customModelDirectoryIds.add(engine.engineId);
    });
    await _scanModelDirectory(engine, selectedPath);
  }

  Future<void> _useDefaultModelDirectory(LocalEngine engine) async {
    if (_registeringEngineId != null || _scanningEngineId != null) return;
    try {
      await widget.settingsPreferences.writeLocalModelDirectory(
        engine.engineId,
        null,
      );
    } on PlatformException {
      if (mounted) setState(() => _modelActionError = 'preferences');
      return;
    }

    setState(() {
      _modelActionError = null;
      _modelDirectories[engine.engineId] = engine.modelDirectory;
      _customModelDirectoryIds.remove(engine.engineId);
    });
    await _scanModelDirectory(engine, engine.modelDirectory);
  }

  Future<void> _scanModelDirectory(
    LocalEngine engine,
    String modelDirectory,
  ) async {
    final service = widget.serviceClient;
    if (service == null ||
        _registeringEngineId != null ||
        _scanningEngineId != null) {
      return;
    }

    setState(() {
      _modelActionError = null;
      _scanningEngineId = engine.engineId;
    });
    late final LocalModelDiscovery discovery;
    try {
      discovery = await LocalEnginesRepository(service).discoverModels(
        engineId: engine.engineId,
        modelDirectory: modelDirectory,
      );
    } on OpenChatServiceException catch (error) {
      if (mounted) setState(() => _modelActionError = error.code);
      return;
    } on FormatException {
      if (mounted) setState(() => _modelActionError = 'invalid_response');
      return;
    } on PlatformException {
      if (mounted) setState(() => _modelActionError = 'discovery_failed');
      return;
    } finally {
      if (mounted) setState(() => _scanningEngineId = null);
    }

    if (!mounted) return;
    if (discovery.models.isEmpty) {
      showOpenChatToast(
        context,
        discovery.truncated
            ? context.openchatL10n.localModelDiscoveryTruncated
            : context.openchatL10n.localModelDiscoveryEmpty,
        type: discovery.truncated
            ? OpenChatToastType.warning
            : OpenChatToastType.info,
      );
      return;
    }

    final shouldRegister = await LocalModelDiscoveryDialog.show(
      context,
      discovery,
    );
    if (shouldRegister != true || !mounted) return;
    setState(() => _registeringEngineId = engine.engineId);

    var registeredCount = 0;
    String? failureCode;
    try {
      for (final model in discovery.models) {
        try {
          await LocalEnginesRepository(service).registerModel(
            engineId: model.engineId,
            modelPath: model.path,
            modelDirectory: modelDirectory,
            storageAction: LocalModelStorageAction.keep,
          );
          registeredCount++;
        } on OpenChatServiceException catch (error) {
          failureCode = error.code;
          break;
        } on FormatException {
          failureCode = 'invalid_response';
          break;
        } on PlatformException {
          failureCode = 'path';
          break;
        }
      }
    } finally {
      if (mounted) setState(() => _registeringEngineId = null);
      await _loadCatalog();
    }

    if (!mounted) return;
    if (failureCode == null) {
      showOpenChatToast(
        context,
        context.openchatL10n.localModelDiscoveryRegistered(registeredCount),
        type: OpenChatToastType.success,
      );
    } else {
      setState(() => _modelActionError = failureCode);
      showOpenChatToast(
        context,
        context.openchatL10n.localModelDiscoveryPartial(
          registeredCount,
          discovery.models.length,
        ),
        type: OpenChatToastType.warning,
      );
    }
  }

  Future<void> _registerModel(LocalEngine engine) async {
    final service = widget.serviceClient;
    if (service == null ||
        _registeringEngineId != null ||
        _scanningEngineId != null) {
      return;
    }

    String? path;
    try {
      if (engine.engineId == 'llama_cpp') {
        final files = await FilePicker.pickFiles(
          dialogTitle: engine.displayName,
          type: FileType.custom,
          allowedExtensions: const <String>['gguf'],
        );
        path = files.isEmpty ? null : files.first.path;
      } else {
        path = await FilePicker.getDirectoryPath(
          dialogTitle: engine.displayName,
        );
      }
    } on PlatformException {
      if (mounted) setState(() => _modelActionError = 'picker');
      return;
    }
    if (path == null || path.trim().isEmpty) return;
    if (!mounted) return;

    final modelDirectory = _modelDirectoryFor(engine);
    final storageAction = await _chooseModelStorageAction(modelDirectory);
    if (!mounted || storageAction == null) return;

    setState(() {
      _modelActionError = null;
      _registeringEngineId = engine.engineId;
    });
    try {
      await LocalEnginesRepository(service).registerModel(
        engineId: engine.engineId,
        modelPath: path,
        modelDirectory: modelDirectory,
        storageAction: storageAction,
      );
    } on OpenChatServiceException catch (error) {
      if (mounted) setState(() => _modelActionError = error.code);
    } on FormatException {
      if (mounted) setState(() => _modelActionError = 'invalid_response');
    } on PlatformException {
      if (mounted) setState(() => _modelActionError = 'path');
    } finally {
      if (mounted) setState(() => _registeringEngineId = null);
      await _loadCatalog();
    }
  }

  Future<LocalModelStorageAction?> _chooseModelStorageAction(
    String modelDirectory,
  ) {
    return showDialog<LocalModelStorageAction>(
      context: context,
      builder: (dialogContext) {
        final l10n = dialogContext.openchatL10n;
        return AlertDialog(
          title: Text(l10n.localModelStorageChoiceTitle),
          content: SizedBox(
            width: 400,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(l10n.localModelStorageChoiceTarget(modelDirectory)),
                const SizedBox(height: 16),
                _buildStorageActionButton(
                  dialogContext,
                  action: LocalModelStorageAction.move,
                  icon: LucideIcons.folderInput,
                  label: l10n.localModelMoveToFolder,
                ),
                const SizedBox(height: 8),
                _buildStorageActionButton(
                  dialogContext,
                  action: LocalModelStorageAction.copy,
                  icon: LucideIcons.copy,
                  label: l10n.localModelCopyToFolder,
                ),
                const SizedBox(height: 8),
                _buildStorageActionButton(
                  dialogContext,
                  action: LocalModelStorageAction.keep,
                  icon: LucideIcons.folderOpen,
                  label: l10n.localModelKeepInPlace,
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: Text(l10n.cancel),
            ),
          ],
        );
      },
    );
  }

  Widget _buildStorageActionButton(
    BuildContext context, {
    required LocalModelStorageAction action,
    required IconData icon,
    required String label,
  }) {
    return SizedBox(
      width: double.infinity,
      child: OutlinedButton.icon(
        onPressed: () => Navigator.of(context).pop(action),
        icon: Icon(icon),
        label: Text(label),
      ),
    );
  }

  Future<void> _removeModel(LocalRegisteredModel model) async {
    final service = widget.serviceClient;
    if (service == null || _removingModelId != null) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(context.openchatL10n.localModelRemove),
        content: Text(context.openchatL10n.localModelRemoveConfirmation),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(context.openchatL10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(context.openchatL10n.localModelRemove),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;

    setState(() {
      _modelActionError = null;
      _removingModelId = model.id;
    });
    try {
      final removed = await LocalEnginesRepository(service)
          .removeModel(model.id);
      if (!removed) {
        throw const FormatException('Model registration was absent.');
      }
    } on OpenChatServiceException catch (error) {
      if (mounted) setState(() => _modelActionError = error.code);
    } on FormatException {
      if (mounted) setState(() => _modelActionError = 'invalid_response');
    } finally {
      if (mounted) setState(() => _removingModelId = null);
      await _loadCatalog();
    }
  }

  Future<void> _startModel(LocalRegisteredModel model) async {
    final service = widget.serviceClient;
    if (service == null || !model.isAvailable || _runtimeOperation != null) {
      return;
    }
    setState(() {
      _modelActionError = null;
      _startingModelId = model.id;
      _isCancellingRuntime = false;
    });
    try {
      final operation = await LocalEnginesRepository(service)
          .startModel(model.id);
      if (!mounted) {
        await operation.cancel();
        return;
      }
      setState(() => _runtimeOperation = operation);
      final result = await operation.result;
      if (result.status != 'running') {
        throw const FormatException('Local model health check did not pass.');
      }
    } on OpenChatServiceException catch (error) {
      if (mounted && error.code != 'operation_cancelled') {
        setState(() => _modelActionError = error.code);
      }
    } on FormatException {
      if (mounted) setState(() => _modelActionError = 'start_failed');
    } finally {
      if (mounted) {
        setState(() {
          _runtimeOperation = null;
          _startingModelId = null;
          _isCancellingRuntime = false;
        });
      }
      await _loadCatalog();
    }
  }

  Future<void> _stopModel(String modelId) async {
    final service = widget.serviceClient;
    if (service == null || _startingModelId != null) return;
    setState(() {
      _modelActionError = null;
      _stoppingModelId = modelId;
    });
    try {
      await LocalEnginesRepository(service).stopModel(modelId);
    } on OpenChatServiceException catch (error) {
      if (mounted) setState(() => _modelActionError = error.code);
    } on FormatException {
      if (mounted) setState(() => _modelActionError = 'start_failed');
    } finally {
      if (mounted) setState(() => _stoppingModelId = null);
      await _loadCatalog();
    }
  }

  Future<void> _cancelRuntimeStart() async {
    final operation = _runtimeOperation;
    if (operation == null || _isCancellingRuntime) return;
    setState(() => _isCancellingRuntime = true);
    final cancelled = await operation.cancel();
    if (!cancelled && mounted) setState(() => _isCancellingRuntime = false);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          l10n.localEnginesDescription,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 13,
            height: 1.45,
          ),
        ),
        const SizedBox(height: 16),
        if (_installError != null) ...[
          _buildMessageCard(
            context,
            icon: LucideIcons.circleAlert,
            message: l10n.localEngineInstallFailed,
            action: OutlinedButton.icon(
              onPressed: _loadState == _LocalEnginesLoadState.loading
                  ? null
                  : () => unawaited(_loadCatalog()),
              icon: const Icon(LucideIcons.refreshCw, size: 16),
              label: Text(l10n.localEnginesReload),
            ),
          ),
          const SizedBox(height: 12),
        ],
        if (_modelActionError != null) ...[
          _buildMessageCard(
            context,
            icon: LucideIcons.circleAlert,
            message: _modelActionErrorLabel(l10n, _modelActionError!),
            action: TextButton(
              onPressed: () => setState(() => _modelActionError = null),
              child: Text(l10n.close),
            ),
          ),
          const SizedBox(height: 12),
        ],
        switch (_loadState) {
          _LocalEnginesLoadState.loading => const LinearProgressIndicator(),
          _LocalEnginesLoadState.unavailable => _buildMessageCard(
            context,
            icon: LucideIcons.microchip,
            message: l10n.localEnginesUnavailable,
          ),
          _LocalEnginesLoadState.failed => _buildMessageCard(
            context,
            icon: LucideIcons.circleAlert,
            message: l10n.localEnginesLoadFailed,
            action: OutlinedButton.icon(
              onPressed: () => unawaited(_loadCatalog()),
              icon: const Icon(LucideIcons.refreshCw, size: 16),
              label: Text(l10n.retry),
            ),
          ),
          _LocalEnginesLoadState.loaded => _buildLoadedState(context),
        },
      ],
    );
  }

  Widget _buildLoadedState(BuildContext context) {
    final l10n = context.openchatL10n;
    final catalog = _catalog;
    if (catalog == null || catalog.engines.isEmpty) {
      return _buildMessageCard(
        context,
        icon: LucideIcons.microchip,
        message: l10n.localEnginesEmpty,
        action: OutlinedButton.icon(
          onPressed: () => unawaited(_loadCatalog()),
          icon: const Icon(LucideIcons.refreshCw, size: 16),
          label: Text(l10n.localEnginesReload),
        ),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (var index = 0; index < catalog.engines.length; index++) ...[
          if (index > 0) const SizedBox(height: 14),
          _buildEngineCard(context, catalog.engines[index]),
        ],
      ],
    );
  }

  Widget _buildEngineCard(BuildContext context, LocalEngine engine) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final isBlocked = engine.catalogStatus == 'blocked';
    final isDeprecated = engine.catalogStatus == 'deprecated';

    return Card(
      margin: EdgeInsets.zero,
      elevation: 0,
      color: palette.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(OpenChatRadii.card),
        side: BorderSide(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                LocalEngineIcon(
                  engineId: engine.engineId,
                  color: palette.accentIcon,
                  size: 24,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Wrap(
                        spacing: 8,
                        runSpacing: 6,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: [
                          Text(
                            engine.displayName,
                            style: Theme.of(context).textTheme.titleMedium,
                          ),
                          _buildBadge(
                            context,
                            _channelLabel(l10n, engine.channel),
                            emphasized: engine.channel != 'stable',
                          ),
                          if (isDeprecated)
                            _buildBadge(context, l10n.localEngineDeprecated),
                        ],
                      ),
                      const SizedBox(height: 4),
                      Text(
                        l10n.localEngineRelease(engine.releaseTag),
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 12,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
            if ((isBlocked || isDeprecated) && engine.statusReason != null) ...[
              const SizedBox(height: 14),
              _buildReasonBanner(context, _statusReasonLabel(l10n, engine)),
            ],
            if (engine.engineId == 'llama_cpp') ...[
              const SizedBox(height: 16),
              _buildLlamaServerExecutable(context),
            ],
            const SizedBox(height: 14),
            Text(
              l10n.localEngineVariants,
              style: TextStyle(
                color: palette.secondaryText,
                fontSize: 12,
                fontWeight: FontWeight.w600,
              ),
            ),
            const SizedBox(height: 8),
            for (var index = 0; index < engine.variants.length; index++) ...[
              if (index > 0) const SizedBox(height: 8),
              _buildVariantTile(context, engine, engine.variants[index]),
            ],
            const SizedBox(height: 16),
            _buildRuntimeStatus(context, engine),
            const SizedBox(height: 16),
            _buildRegisteredModels(context, engine),
          ],
        ),
      ),
    );
  }

  Widget _buildRuntimeStatus(BuildContext context, LocalEngine engine) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final installed =
        engine.variants.any((variant) => variant.installed) ||
        (engine.engineId == 'llama_cpp' &&
            (_catalog?.llamaServerExecutableAvailable ?? false));
    final statusLabel = engine.catalogStatus == 'blocked'
        ? l10n.localEngineUnavailable
        : !installed
        ? l10n.localEngineNotInstalled
        : switch (engine.runtimeStatus) {
            'running' => l10n.localEngineRunning,
            'stopped' => l10n.localEngineStopped,
            'unhealthy' => l10n.localEngineUnhealthy,
            _ => l10n.localEngineUnavailable,
          };
    final statusEmphasized = installed && engine.runtimeStatus == 'running';

    return Row(
      children: [
        Expanded(
          child: Text(
            l10n.localEngineHealth,
            style: TextStyle(
              color: palette.secondaryText,
              fontSize: 12,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
        _buildBadge(context, statusLabel, emphasized: statusEmphasized),
      ],
    );
  }

  Widget _buildLlamaServerExecutable(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final catalog = _catalog;
    final path = catalog?.llamaServerExecutablePath;
    final available = catalog?.llamaServerExecutableAvailable ?? false;
    final externalServers = catalog?.externalLlamaServers ?? const [];

    return DecoratedBox(
      decoration: BoxDecoration(
        color: palette.composer,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              l10n.localEngineExecutable,
              style: Theme.of(context).textTheme.titleSmall,
            ),
            const SizedBox(height: 4),
            Text(
              l10n.localEngineExecutableDescription,
              style: TextStyle(color: palette.secondaryText, fontSize: 12),
            ),
            const SizedBox(height: 10),
            if (externalServers.isNotEmpty) ...[
              for (final server in externalServers) ...[
                DecoratedBox(
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(7),
                    border: Border.all(color: palette.border),
                  ),
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(10, 6, 4, 6),
                    child: Row(
                      children: [
                        Icon(
                          LucideIcons.link,
                          size: 16,
                          color: palette.accentIcon,
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            l10n.localEngineExternalServerConnected(
                              server.port,
                              server.modelIds.length,
                            ),
                            style: TextStyle(color: palette.text, fontSize: 12),
                          ),
                        ),
                        TextButton(
                          onPressed: _isDisconnectingExternalServer
                              ? null
                              : () => unawaited(
                                  _disconnectExternalLlamaServer(server),
                                ),
                          child: _isDisconnectingExternalServer
                              ? const SizedBox.square(
                                  dimension: 16,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : Text(l10n.localEngineExternalServerDisconnect),
                        ),
                      ],
                    ),
                  ),
                ),
                if (server != externalServers.last) const SizedBox(height: 6),
              ],
            ] else if (_isConnectingExternalServer) ...[
              const LinearProgressIndicator(),
              const SizedBox(height: 8),
              Text(
                l10n.localEngineExternalServerConnecting,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
            ] else
              Text(
                l10n.localEngineExternalServerNotConnected,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
            const SizedBox(height: 8),
            if (path == null)
              Text(
                l10n.localEngineExecutableNotConfigured,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              )
            else ...[
              Tooltip(
                message: path,
                child: Text(
                  available ? path : l10n.localEngineExecutableMissing,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: available ? palette.text : palette.secondaryText,
                    fontSize: 12,
                  ),
                ),
              ),
              const SizedBox(height: 8),
            ],
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                if (defaultTargetPlatform == TargetPlatform.windows)
                  OutlinedButton.icon(
                    onPressed: _isDetectingExternalServer
                        ? null
                        : () => unawaited(
                            _detectExternalLlamaServers(allowRepeat: true),
                          ),
                    icon: _isDetectingExternalServer
                        ? const SizedBox.square(
                            dimension: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(LucideIcons.radar, size: 16),
                    label: Text(l10n.localEngineExternalServerCheck),
                  ),
                OutlinedButton.icon(
                  onPressed: _isSavingExecutablePath
                      ? null
                      : () => unawaited(_chooseLlamaServerExecutable()),
                  icon: _isSavingExecutablePath
                      ? const SizedBox.square(
                          dimension: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(LucideIcons.folderOpen, size: 16),
                  label: Text(l10n.localEngineExecutableChoose),
                ),
                if (path != null)
                  TextButton.icon(
                    onPressed: _isSavingExecutablePath
                        ? null
                        : () => unawaited(_saveLlamaServerExecutable(null)),
                    icon: const Icon(LucideIcons.x, size: 16),
                    label: Text(l10n.localEngineExecutableClear),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _chooseLlamaServerExecutable() async {
    try {
      final files = await FilePicker.pickFiles(
        dialogTitle: context.openchatL10n.localEngineExecutableChoose,
        type: defaultTargetPlatform == TargetPlatform.windows
            ? FileType.custom
            : FileType.any,
        allowedExtensions: defaultTargetPlatform == TargetPlatform.windows
            ? const <String>['exe']
            : null,
      );
      if (files.isEmpty || files.first.path == null) return;
      await _saveLlamaServerExecutable(files.first.path);
    } on PlatformException {
      if (mounted) setState(() => _modelActionError = 'picker');
    }
  }

  Future<void> _saveLlamaServerExecutable(String? path) async {
    final service = widget.serviceClient;
    if (service == null || _isSavingExecutablePath) return;
    setState(() {
      _isSavingExecutablePath = true;
      _modelActionError = null;
    });
    try {
      await LocalEnginesRepository(service).setLlamaServerExecutablePath(path);
      await _loadCatalog();
    } on OpenChatServiceException catch (error) {
      if (mounted) setState(() => _modelActionError = error.code);
    } on FormatException {
      if (mounted) setState(() => _modelActionError = 'invalid_response');
    } on PlatformException {
      if (mounted) setState(() => _modelActionError = 'settings');
    } finally {
      if (mounted) setState(() => _isSavingExecutablePath = false);
    }
  }

  Widget _buildRegisteredModels(BuildContext context, LocalEngine engine) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final models = (_catalog?.models ?? const <LocalRegisteredModel>[])
        .where((model) => model.engineId == engine.engineId)
        .toList(growable: false);
    final isRegistering = _registeringEngineId == engine.engineId;
    final addLabel = engine.engineId == 'llama_cpp'
        ? l10n.localModelAddFile
        : l10n.localModelAddFolder;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        LocalModelDirectoryControls(
          directory: _modelDirectoryFor(engine),
          isCustom: _customModelDirectoryIds.contains(engine.engineId),
          isBusy: _registeringEngineId != null || _scanningEngineId != null,
          isScanning: _scanningEngineId == engine.engineId,
          onChoose: () => unawaited(_chooseModelDirectory(engine)),
          onUseDefault: () => unawaited(_useDefaultModelDirectory(engine)),
          onScan: () => unawaited(
            _scanModelDirectory(engine, _modelDirectoryFor(engine)),
          ),
        ),
        const SizedBox(height: 16),
        Wrap(
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: 12,
          runSpacing: 8,
          children: [
            Text(
              l10n.localModels,
              style: TextStyle(
                color: palette.secondaryText,
                fontSize: 12,
                fontWeight: FontWeight.w600,
              ),
            ),
            OutlinedButton.icon(
              onPressed:
                  isRegistering ||
                      _registeringEngineId != null ||
                      _scanningEngineId != null
                  ? null
                  : () => unawaited(_registerModel(engine)),
              icon: isRegistering
                  ? const SizedBox.square(
                      dimension: 15,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.plus, size: 17),
              label: Text(isRegistering ? l10n.localModelSaving : addLabel),
            ),
          ],
        ),
        const SizedBox(height: 8),
        if (models.isEmpty)
          Text(
            l10n.localModelsEmpty,
            style: TextStyle(color: palette.secondaryText, fontSize: 12),
          )
        else
          for (var index = 0; index < models.length; index++) ...[
            if (index > 0) const SizedBox(height: 8),
            _buildRegisteredModel(context, models[index]),
          ],
      ],
    );
  }

  Widget _buildRegisteredModel(
    BuildContext context,
    LocalRegisteredModel model,
  ) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final runtime = _catalog?.runtime;
    final runtimeServers = runtime?.servers ?? const [];
    final isRunning = runtimeServers.isNotEmpty
        ? runtimeServers.any(
            (server) =>
                server.modelId == model.id && server.status == 'running',
          )
        : runtime?.status == 'running' && runtime?.modelId == model.id;
    final isStarting = _startingModelId == model.id;
    final isStopping = _stoppingModelId == model.id;
    final isRemoving = _removingModelId == model.id;
    final isBusy =
        _runtimeOperation != null ||
        _startingModelId != null ||
        _stoppingModelId != null ||
        _removingModelId != null;
    final modelState = !model.pathExists
        ? l10n.localModelPathMissing
        : model.isAvailable
        ? null
        : l10n.localModelEngineNotReady;

    return DecoratedBox(
      decoration: BoxDecoration(
        color: palette.composer,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 10, 8, 10),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    model.displayName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: model.isAvailable
                          ? palette.text
                          : palette.secondaryText,
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  const SizedBox(height: 3),
                  Tooltip(
                    message: model.path,
                    child: Text(
                      modelState ?? model.path,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: OpenChatTypography.metadata,
                      ),
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            if (isStarting || isStopping)
              IconButton(
                tooltip: isStopping
                    ? l10n.localModelStopping
                    : l10n.localModelCancelStart,
                onPressed:
                    isStarting &&
                        _runtimeOperation != null &&
                        !_isCancellingRuntime
                    ? () => unawaited(_cancelRuntimeStart())
                    : null,
                icon: SizedBox.square(
                  dimension: 16,
                  child: isStopping || _isCancellingRuntime
                      ? const CircularProgressIndicator(strokeWidth: 2)
                      : const Icon(LucideIcons.x, size: 16),
                ),
              )
            else if (isRunning)
              IconButton(
                tooltip: l10n.localEngineStopModel,
                onPressed: isBusy
                    ? null
                    : () => unawaited(_stopModel(model.id)),
                icon: isStopping
                    ? const SizedBox.square(
                        dimension: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(LucideIcons.circleStop),
              )
            else
              IconButton(
                tooltip: l10n.localEngineStartModel,
                onPressed: !model.isAvailable || isBusy
                    ? null
                    : () => unawaited(_startModel(model)),
                icon: const Icon(LucideIcons.circlePlay),
              ),
            IconButton(
              tooltip: l10n.localModelRemove,
              onPressed: isBusy ? null : () => unawaited(_removeModel(model)),
              icon: isRemoving
                  ? const SizedBox.square(
                      dimension: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.trash2),
            ),
          ],
        ),
      ),
    );
  }

  String _modelActionErrorLabel(AppLocalizations l10n, String code) =>
      switch (code) {
        'local_model_format_invalid' => l10n.localModelInvalid,
        'local_model_path_unavailable' ||
        'picker' ||
        'path' => l10n.localModelPathError,
        'local_model_directory_unavailable' ||
        'local_model_storage_path_unavailable' =>
          l10n.localModelDirectoryUnavailable,
        'local_model_discovery_failed' ||
        'discovery_failed' => l10n.localModelDiscoveryFailed,
        'local_model_storage_unavailable' => l10n.localModelStorageError,
        'local_model_transfer_failed' => l10n.localModelTransferError,
        'local_model_transfer_recovery_needed' =>
          l10n.localModelTransferRecoveryError,
        'local_engine_not_installed' ||
        'local_engine_unavailable' ||
        'local_engine_variant_unavailable' ||
        'local_engine_install_unavailable' ||
        'local_model_unavailable' => l10n.localModelEngineNotReady,
        'local_engine_start_timeout' => l10n.localModelStartTimeout,
        'local_engine_executable_path_invalid' =>
          l10n.localEngineExecutableInvalid,
        'local_engine_settings_unavailable' ||
        'settings' => l10n.localEngineSettingsFailed,
        'local_engine_process_scan_failed' ||
        'scan_failed' => l10n.localEngineExternalServerScanFailed,
        'local_engine_external_server_auth_required' =>
          l10n.localEngineExternalServerAuthRequired,
        'local_engine_external_server_not_found' ||
        'local_engine_external_server_not_ready' ||
        'local_engine_external_server_catalog_invalid' ||
        'local_engine_external_server_capabilities_invalid' ||
        'connect_failed' => l10n.localEngineExternalServerConnectFailed,
        'start_failed' ||
        'local_engine_start_failed' ||
        'local_engine_runtime_unavailable' => l10n.localModelStartError,
        _ => l10n.localModelActionError,
      };

  Widget _buildVariantTile(
    BuildContext context,
    LocalEngine engine,
    LocalEngineVariant variant,
  ) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final isInstalling =
        _installingEngineId == engine.engineId &&
        _installingVariantId == variant.variantId;
    final canStart =
        variant.canInstall &&
        _activeOperation == null &&
        _installingEngineId == null;

    return DecoratedBox(
      decoration: BoxDecoration(
        color: palette.composer,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 11, 12, 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Wrap(
                    spacing: 6,
                    runSpacing: 6,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      Text(
                        _acceleratorLabel(variant.accelerator),
                        style: TextStyle(
                          color: palette.text,
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      _buildBadge(context, variant.os),
                      _buildBadge(context, variant.architecture),
                      if (variant.recommended)
                        _buildBadge(
                          context,
                          l10n.localEngineRecommended,
                          emphasized: true,
                        ),
                    ],
                  ),
                ),
                const SizedBox(width: 8),
                Text(
                  _variantStatusLabel(l10n, variant.status),
                  style: TextStyle(
                    color: variant.installed
                        ? palette.accent
                        : palette.secondaryText,
                    fontSize: 12,
                  ),
                ),
              ],
            ),
            if (variant.runtimeRequirements.isNotEmpty) ...[
              const SizedBox(height: 8),
              Text(
                l10n.localEngineRuntimeRequirements(
                  variant.runtimeRequirements.join(' · '),
                ),
                style: TextStyle(
                  color: palette.secondaryText,
                  fontSize: 12,
                  height: 1.4,
                ),
              ),
            ],
            if (isInstalling) ...[
              const SizedBox(height: 12),
              _buildInstallProgress(context),
            ] else if (variant.canInstall) ...[
              const SizedBox(height: 12),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton.icon(
                  onPressed: canStart
                      ? () => unawaited(_install(engine, variant))
                      : null,
                  icon: const Icon(LucideIcons.download, size: 16),
                  label: Text(l10n.localEngineInstall),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _buildInstallProgress(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final latest = _latestProgress;
    final fraction = latest?.fraction;
    final assetProgress = _assetProgress.values.toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                latest == null
                    ? l10n.localEngineInstalling
                    : _stageLabel(l10n, latest.stage),
                style: TextStyle(
                  color: palette.text,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            if (latest != null && latest.stage != 'setting_up_runtime')
              Text(
                latest.assetName,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
          ],
        ),
        const SizedBox(height: 8),
        Semantics(
          label: l10n.localEngineInstallProgress,
          value: fraction == null ? null : '${(fraction * 100).round()}%',
          child: LinearProgressIndicator(value: fraction),
        ),
        if (latest != null && latest.stage != 'setting_up_runtime') ...[
          const SizedBox(height: 6),
          Text(
            _progressDetails(l10n, latest),
            style: TextStyle(
              color: palette.secondaryText,
              fontSize: OpenChatTypography.metadata,
            ),
          ),
        ],
        if (assetProgress.length > 1) ...[
          const SizedBox(height: 10),
          for (final progress in assetProgress) ...[
            _buildAssetProgressRow(context, progress),
            if (progress != assetProgress.last) const SizedBox(height: 6),
          ],
        ],
        const SizedBox(height: 10),
        Align(
          alignment: Alignment.centerRight,
          child: TextButton.icon(
            onPressed: _activeOperation == null || _isCancelling
                ? null
                : _cancelInstall,
            icon: _isCancelling
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(LucideIcons.x, size: 16),
            label: Text(
              _isCancelling
                  ? l10n.localEngineCancellingInstall
                  : l10n.localEngineCancelInstall,
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildAssetProgressRow(
    BuildContext context,
    LocalEngineInstallProgress progress,
  ) {
    final palette = OpenChatPalette.of(context);
    final fraction = progress.fraction;
    return Row(
      children: [
        Expanded(
          child: Text(
            progress.assetName,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: palette.secondaryText,
              fontSize: OpenChatTypography.metadata,
            ),
          ),
        ),
        const SizedBox(width: 8),
        SizedBox(
          width: 92,
          child: LinearProgressIndicator(value: fraction, minHeight: 3),
        ),
      ],
    );
  }

  Widget _buildMessageCard(
    BuildContext context, {
    required IconData icon,
    required String message,
    Widget? action,
  }) {
    final palette = OpenChatPalette.of(context);
    return Card(
      margin: EdgeInsets.zero,
      elevation: 0,
      color: palette.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
        side: BorderSide(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Icon(icon, color: palette.secondaryIcon, size: 20),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                message,
                style: TextStyle(color: palette.secondaryText, fontSize: 13),
              ),
            ),
            if (action != null) ...[const SizedBox(width: 12), action],
          ],
        ),
      ),
    );
  }

  Widget _buildReasonBanner(BuildContext context, String reason) {
    final palette = OpenChatPalette.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: palette.hover,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(10),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(LucideIcons.info, color: palette.accentIcon, size: 17),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                reason,
                style: TextStyle(
                  color: palette.text,
                  fontSize: 12,
                  height: 1.4,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildBadge(
    BuildContext context,
    String label, {
    bool emphasized = false,
  }) {
    final palette = OpenChatPalette.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: emphasized
            ? palette.accent.withValues(alpha: 0.12)
            : palette.hover,
        borderRadius: BorderRadius.circular(999),
        border: Border.all(
          color: emphasized
              ? palette.accent.withValues(alpha: 0.35)
              : palette.border,
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
        child: Text(
          label,
          style: TextStyle(
            color: emphasized ? palette.accent : palette.secondaryText,
            fontSize: OpenChatTypography.metadata,
            fontWeight: FontWeight.w600,
          ),
        ),
      ),
    );
  }

  String _channelLabel(AppLocalizations l10n, String channel) =>
      switch (channel) {
        'stable' => l10n.localEngineStable,
        'preview' => l10n.localEnginePreview,
        'nightly' => l10n.localEngineNightly,
        _ => channel,
      };

  String _statusReasonLabel(AppLocalizations l10n, LocalEngine engine) =>
      engine.catalogStatus == 'deprecated'
      ? l10n.localEngineWindowsDeprecatedReason
      : switch (engine.engineId) {
          'vllm' => l10n.localEngineVllmBlockedReason,
          'exllama' => l10n.localEngineExllamaBlockedReason,
          _ => engine.statusReason ?? l10n.localEngineBlocked,
        };

  String _variantStatusLabel(AppLocalizations l10n, String status) =>
      switch (status) {
        'available' => l10n.localEngineAvailable,
        'installed' => l10n.localEngineInstalled,
        'blocked' => l10n.localEngineBlocked,
        'deprecated' => l10n.localEngineDeprecated,
        'unsupported_platform' => l10n.localEngineUnsupportedPlatform,
        'hardware_unavailable' => l10n.localEngineHardwareUnavailable,
        'driver_unsupported' => l10n.localEngineDriverUnsupported,
        'driver_version_unavailable' =>
          l10n.localEngineDriverVersionUnavailable,
        _ => status,
      };

  String _stageLabel(AppLocalizations l10n, String stage) => switch (stage) {
    'downloading' => l10n.localEngineStageDownloading,
    'verifying' => l10n.localEngineStageVerifying,
    'extracting' => l10n.localEngineStageExtracting,
    'setting_up_runtime' => l10n.localEngineStageRuntimeSetup,
    'publishing' => l10n.localEngineStagePublishing,
    'ready' => l10n.localEngineStageReady,
    _ => stage,
  };

  String _progressDetails(
    AppLocalizations l10n,
    LocalEngineInstallProgress progress,
  ) {
    final bytes =
        '${_formatBytes(progress.downloadedBytes)} / ${_formatBytes(progress.totalBytes)}';
    final index = progress.assetIndex != null && progress.assetCount != null
        ? ' · ${progress.assetIndex! + 1}/${progress.assetCount}'
        : '';
    return '${_stageLabel(l10n, progress.stage)} · $bytes$index';
  }

  String _acceleratorLabel(String accelerator) => switch (accelerator) {
    'cuda' => 'CUDA',
    'vulkan' => 'Vulkan',
    'metal' => 'Metal',
    'xpu' => 'XPU',
    'cpu' => 'CPU',
    _ => accelerator,
  };

  String _formatBytes(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) {
      return '${(bytes / 1024).toStringAsFixed(1)} KB';
    }
    if (bytes < 1024 * 1024 * 1024) {
      return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }
}
