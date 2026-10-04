import 'package:openchat/platform/windows/openchat_service_client.dart';

class LocalEngineCatalog {
  const LocalEngineCatalog({
    required this.engines,
    required this.models,
    required this.runtime,
  });

  final List<LocalEngine> engines;
  final List<LocalRegisteredModel> models;
  final LocalEngineRuntimeState runtime;

  factory LocalEngineCatalog.fromJson(Map<String, Object?> json) {
    final rawEngines = json['engines'];
    if (rawEngines is! List<Object?>) {
      throw const FormatException(
        'Local engine catalog did not contain engines.',
      );
    }
    final rawModels = json['models'];
    if (rawModels is! List<Object?>) {
      throw const FormatException(
        'Local engine catalog did not contain registered models.',
      );
    }

    return LocalEngineCatalog(
      engines: rawEngines
          .map((value) => LocalEngine.fromJson(_objectMap(value)))
          .toList(growable: false),
      models: rawModels
          .map((value) => LocalRegisteredModel.fromJson(_objectMap(value)))
          .toList(growable: false),
      runtime: LocalEngineRuntimeState.fromJson(_objectMap(json['runtime'])),
    );
  }
}

class LocalEngine {
  const LocalEngine({
    required this.engineId,
    required this.displayName,
    required this.releaseTag,
    required this.channel,
    required this.catalogStatus,
    required this.statusReason,
    required this.runtimeStatus,
    required this.modelDirectory,
    required this.variants,
  });

  final String engineId;
  final String displayName;
  final String releaseTag;
  final String channel;
  final String catalogStatus;
  final String? statusReason;
  final String runtimeStatus;
  final String modelDirectory;
  final List<LocalEngineVariant> variants;

  factory LocalEngine.fromJson(Map<String, Object?> json) {
    final rawVariants = json['variants'];
    if (rawVariants is! List<Object?>) {
      throw const FormatException(
        'Local engine entry did not contain variants.',
      );
    }

    return LocalEngine(
      engineId: _requiredString(json, 'engineId'),
      displayName: _requiredString(json, 'displayName'),
      releaseTag: _requiredString(json, 'releaseTag'),
      channel: _requiredString(json, 'channel'),
      catalogStatus: _requiredString(json, 'catalogStatus'),
      statusReason: _optionalString(json, 'statusReason'),
      runtimeStatus: _requiredString(json, 'runtimeStatus'),
      modelDirectory: _requiredString(json, 'modelDirectory'),
      variants: rawVariants
          .map((value) => LocalEngineVariant.fromJson(_objectMap(value)))
          .toList(growable: false),
    );
  }
}

class LocalRegisteredModel {
  const LocalRegisteredModel({
    required this.id,
    required this.engineId,
    required this.displayName,
    required this.path,
    required this.pathKind,
    required this.pathExists,
    required this.isAvailable,
    this.reason,
  });

  final String id;
  final String engineId;
  final String displayName;
  final String path;
  final String pathKind;
  final bool pathExists;
  final bool isAvailable;
  final String? reason;

  factory LocalRegisteredModel.fromJson(Map<String, Object?> json) {
    return LocalRegisteredModel(
      id: _requiredString(json, 'id'),
      engineId: _requiredString(json, 'engineId'),
      displayName: _requiredString(json, 'displayName'),
      path: _requiredString(json, 'path'),
      pathKind: _requiredString(json, 'pathKind'),
      pathExists: _requiredBool(json, 'pathExists'),
      isAvailable: _requiredBool(json, 'isAvailable'),
      reason: _optionalString(json, 'reason'),
    );
  }
}

class LocalModelCatalog {
  const LocalModelCatalog({required this.models});

  final List<LocalRegisteredModel> models;

  factory LocalModelCatalog.fromJson(Map<String, Object?> json) {
    if (json['freshness'] != 'current') {
      throw const FormatException('Local model catalog freshness was invalid.');
    }
    final rawModels = json['models'];
    if (rawModels is! List<Object?>) {
      throw const FormatException(
        'Local model catalog did not contain models.',
      );
    }
    return LocalModelCatalog(
      models: rawModels
          .map((value) => LocalRegisteredModel.fromJson(_objectMap(value)))
          .toList(growable: false),
    );
  }
}

class LocalModelDiscovery {
  const LocalModelDiscovery({required this.models, required this.truncated});

  final List<LocalDiscoveredModel> models;
  final bool truncated;

  factory LocalModelDiscovery.fromJson(Map<String, Object?> json) {
    final rawModels = json['models'];
    final truncated = json['truncated'];
    if (rawModels is! List<Object?> || truncated is! bool) {
      throw const FormatException(
        'Local model discovery response was invalid.',
      );
    }
    return LocalModelDiscovery(
      models: rawModels
          .map((value) => LocalDiscoveredModel.fromJson(_objectMap(value)))
          .toList(growable: false),
      truncated: truncated,
    );
  }
}

class LocalDiscoveredModel {
  const LocalDiscoveredModel({
    required this.engineId,
    required this.displayName,
    required this.path,
    required this.pathKind,
  });

  final String engineId;
  final String displayName;
  final String path;
  final String pathKind;

  String get key => '$engineId:$path';

  factory LocalDiscoveredModel.fromJson(Map<String, Object?> json) {
    final model = LocalDiscoveredModel(
      engineId: _requiredString(json, 'engineId'),
      displayName: _requiredString(json, 'displayName'),
      path: _requiredString(json, 'path'),
      pathKind: _requiredString(json, 'pathKind'),
    );
    if (!const <String>{
          'llama_cpp',
          'exllama',
          'vllm',
        }.contains(model.engineId) ||
        !const <String>{'file', 'directory'}.contains(model.pathKind)) {
      throw const FormatException(
        'Discovered local model metadata was invalid.',
      );
    }
    return model;
  }
}

class LocalEngineRuntimeState {
  const LocalEngineRuntimeState({
    required this.status,
    this.engineId,
    this.modelId,
    this.port,
  });

  final String status;
  final String? engineId;
  final String? modelId;
  final int? port;

  factory LocalEngineRuntimeState.fromJson(Map<String, Object?> json) {
    return LocalEngineRuntimeState(
      status: _requiredString(json, 'status'),
      engineId: _optionalString(json, 'engineId'),
      modelId: _optionalString(json, 'modelId'),
      port: _optionalNonNegativeInt(json, 'port'),
    );
  }
}

class LocalEngineVariant {
  const LocalEngineVariant({
    required this.variantId,
    required this.os,
    required this.architecture,
    required this.accelerator,
    required this.runtimeRequirements,
    required this.canInstall,
    required this.recommended,
    required this.installed,
    required this.status,
  });

  final String variantId;
  final String os;
  final String architecture;
  final String accelerator;
  final List<String> runtimeRequirements;
  final bool canInstall;
  final bool recommended;
  final bool installed;
  final String status;

  factory LocalEngineVariant.fromJson(Map<String, Object?> json) {
    return LocalEngineVariant(
      variantId: _requiredString(json, 'variantId'),
      os: _requiredString(json, 'os'),
      architecture: _requiredString(json, 'architecture'),
      accelerator: _requiredString(json, 'accelerator'),
      runtimeRequirements: _requiredStringList(json, 'runtimeRequirements'),
      canInstall: _requiredBool(json, 'canInstall'),
      recommended: _requiredBool(json, 'recommended'),
      installed: _requiredBool(json, 'installed'),
      status: _requiredString(json, 'status'),
    );
  }
}

class LocalEngineInstallProgress {
  const LocalEngineInstallProgress({
    required this.engineId,
    required this.variantId,
    required this.stage,
    required this.assetName,
    required this.downloadedBytes,
    required this.totalBytes,
    this.assetIndex,
    this.assetCount,
  });

  final String engineId;
  final String variantId;
  final String stage;
  final String assetName;
  final int downloadedBytes;
  final int totalBytes;
  final int? assetIndex;
  final int? assetCount;

  double? get fraction {
    if (stage == 'setting_up_runtime') return null;
    if (totalBytes <= 0) return null;
    return (downloadedBytes / totalBytes).clamp(0, 1).toDouble();
  }

  factory LocalEngineInstallProgress.fromJson(Map<String, Object?> json) {
    final stage =
        _optionalString(json, 'stage') ?? _optionalString(json, 'phase');
    if (stage == null) {
      throw const FormatException(
        'Local engine progress did not contain a stage.',
      );
    }

    return LocalEngineInstallProgress(
      engineId: _requiredString(json, 'engineId'),
      variantId: _requiredString(json, 'variantId'),
      stage: stage,
      assetName: _requiredString(json, 'assetName'),
      downloadedBytes: _requiredNonNegativeInt(json, 'downloadedBytes'),
      totalBytes: _requiredNonNegativeInt(json, 'totalBytes'),
      assetIndex: _optionalNonNegativeInt(json, 'assetIndex'),
      assetCount: _optionalNonNegativeInt(json, 'assetCount'),
    );
  }
}

class LocalEngineInstallResult {
  const LocalEngineInstallResult({
    required this.installed,
    this.engineId,
    this.variantId,
    this.releaseTag,
  });

  final bool installed;
  final String? engineId;
  final String? variantId;
  final String? releaseTag;

  factory LocalEngineInstallResult.fromJson(Map<String, Object?> json) {
    return LocalEngineInstallResult(
      installed: _requiredBool(json, 'installed'),
      engineId: _optionalString(json, 'engineId'),
      variantId: _optionalString(json, 'variantId'),
      releaseTag: _optionalString(json, 'releaseTag'),
    );
  }
}

class LocalEngineInstallOperation {
  const LocalEngineInstallOperation.fromServiceOperation(this._operation);

  final OpenChatServiceOperation _operation;

  Stream<LocalEngineInstallProgress> get progress => _operation.events
      .where((event) => event.name == 'local.engines.install.progress')
      .map((event) => LocalEngineInstallProgress.fromJson(event.data));

  Future<LocalEngineInstallResult> get result async =>
      LocalEngineInstallResult.fromJson(await _operation.result);

  Future<bool> cancel() => _operation.cancel();
}

Map<String, Object?> _objectMap(Object? value) {
  if (value is Map<String, Object?>) return value;
  if (value is Map) {
    final result = <String, Object?>{};
    for (final entry in value.entries) {
      final key = entry.key;
      if (key is! String) {
        throw const FormatException(
          'Expected string keys in a service object.',
        );
      }
      result[key] = entry.value;
    }
    return result;
  }
  throw const FormatException(
    'Expected an object from the local engine service.',
  );
}

String _requiredString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is String && value.trim().isNotEmpty) return value;
  throw FormatException('Local engine response field $key was invalid.');
}

String? _optionalString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value == null) return null;
  if (value is String && value.trim().isNotEmpty) return value;
  throw FormatException('Local engine response field $key was invalid.');
}

bool _requiredBool(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is bool) return value;
  throw FormatException('Local engine response field $key was invalid.');
}

int _requiredNonNegativeInt(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is int && value >= 0) return value;
  if (value is num && value.isFinite && value >= 0 && value == value.round()) {
    return value.toInt();
  }
  throw FormatException('Local engine response field $key was invalid.');
}

int? _optionalNonNegativeInt(Map<String, Object?> json, String key) {
  if (!json.containsKey(key) || json[key] == null) return null;
  return _requiredNonNegativeInt(json, key);
}

List<String> _requiredStringList(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! List<Object?>) {
    throw FormatException('Local engine response field $key was invalid.');
  }
  return value
      .map((item) {
        if (item is String && item.trim().isNotEmpty) return item;
        throw FormatException('Local engine response field $key was invalid.');
      })
      .toList(growable: false);
}
