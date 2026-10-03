import 'dart:async';

import 'package:flutter/foundation.dart';

import 'package:openchat/platform/windows/openchat_service_client.dart';

enum HuggingFaceModelFormat {
  gguf('gguf', 'llama_cpp'),
  transformers('transformers', 'vllm'),
  exllama('exllama', 'exllama');

  const HuggingFaceModelFormat(this.wireValue, this.engineId);

  final String wireValue;
  final String engineId;
}

enum HuggingFaceModelSort {
  downloads('downloads'),
  likes('likes'),
  recentlyUpdated('recently_updated');

  const HuggingFaceModelSort(this.wireValue);

  final String wireValue;
}

class HuggingFaceModelSearchPage {
  const HuggingFaceModelSearchPage({
    required this.models,
    required this.nextCursor,
  });

  final List<HuggingFaceModelSearchResult> models;
  final String? nextCursor;
}

class HuggingFaceModelSearchResult {
  const HuggingFaceModelSearchResult({
    required this.repoId,
    required this.downloads,
    required this.likes,
    required this.tags,
    this.pipelineTag,
    this.libraryName,
    this.license,
    this.gated = false,
    this.private = false,
  });

  final String repoId;
  final int downloads;
  final int likes;
  final List<String> tags;
  final String? pipelineTag;
  final String? libraryName;
  final String? license;
  final bool gated;
  final bool private;

  factory HuggingFaceModelSearchResult.fromJson(Map<String, Object?> json) {
    return HuggingFaceModelSearchResult(
      repoId: _requiredString(json, 'repoId'),
      downloads: _nonNegativeInt(json['downloads']),
      likes: _nonNegativeInt(json['likes']),
      tags: _stringList(json['tags']),
      pipelineTag: _optionalString(json['pipelineTag']),
      libraryName: _optionalString(json['libraryName']),
      license: _optionalString(json['license']),
      gated: _optionalBool(json['gated']) ?? false,
      private: _optionalBool(json['private']) ?? false,
    );
  }
}

class HuggingFaceModelFile {
  const HuggingFaceModelFile({
    required this.path,
    required this.kind,
    this.sizeBytes,
  });

  final String path;
  final HuggingFaceModelFileKind kind;
  final int? sizeBytes;

  factory HuggingFaceModelFile.fromJson(Map<String, Object?> json) {
    final kind = HuggingFaceModelFileKind.fromWire(json['kind']);
    if (kind == null) {
      throw const FormatException(
        'Hugging Face model file had an invalid kind.',
      );
    }
    return HuggingFaceModelFile(
      path: _requiredString(json, 'path'),
      kind: kind,
      sizeBytes: _optionalNonNegativeInt(json['sizeBytes']),
    );
  }
}

enum HuggingFaceModelFileKind {
  model,
  vision,
  mtp,
  auxiliary;

  static HuggingFaceModelFileKind? fromWire(Object? value) => switch (value) {
    'model' => HuggingFaceModelFileKind.model,
    'vision' => HuggingFaceModelFileKind.vision,
    'mtp' => HuggingFaceModelFileKind.mtp,
    'auxiliary' => HuggingFaceModelFileKind.auxiliary,
    _ => null,
  };
}

class HuggingFaceDownloadGroup {
  const HuggingFaceDownloadGroup({
    required this.id,
    required this.displayName,
    required this.files,
    this.totalBytes,
  });

  final String id;
  final String displayName;
  final List<HuggingFaceModelFile> files;
  final int? totalBytes;

  bool get canDownload =>
      files.isNotEmpty &&
      totalBytes != null &&
      files.every((file) => file.sizeBytes != null);

  factory HuggingFaceDownloadGroup.fromJson(Map<String, Object?> json) {
    final rawFiles = json['files'];
    if (rawFiles is! List<Object?>) {
      throw const FormatException(
        'Hugging Face download group had invalid files.',
      );
    }
    return HuggingFaceDownloadGroup(
      id: _requiredString(json, 'id'),
      displayName: _requiredString(json, 'displayName'),
      files: rawFiles
          .map((value) => HuggingFaceModelFile.fromJson(_objectMap(value)))
          .toList(growable: false),
      totalBytes: _optionalNonNegativeInt(json['totalBytes']),
    );
  }
}

enum HuggingFaceModelReadmeStatus {
  available,
  missing,
  accessDenied,
  tooLarge,
  unavailable,
}

class HuggingFaceModelDetails {
  const HuggingFaceModelDetails({
    required this.repoId,
    required this.revision,
    required this.gated,
    required this.private,
    required this.files,
    required this.downloadGroups,
    required this.readmeStatus,
    this.license,
    this.readmeMarkdown,
    this.readmeErrorCode,
  });

  final String repoId;
  final String revision;
  final bool gated;
  final bool private;
  final String? license;
  final List<HuggingFaceModelFile> files;
  final List<HuggingFaceDownloadGroup> downloadGroups;
  final String? readmeMarkdown;
  final HuggingFaceModelReadmeStatus readmeStatus;
  final String? readmeErrorCode;

  factory HuggingFaceModelDetails.fromJson(Map<String, Object?> json) {
    final rawFiles = json['files'];
    final rawGroups = json['downloadGroups'];
    final readmeStatus = switch (json['readmeStatus']) {
      'available' => HuggingFaceModelReadmeStatus.available,
      'missing' => HuggingFaceModelReadmeStatus.missing,
      'access_denied' => HuggingFaceModelReadmeStatus.accessDenied,
      'too_large' => HuggingFaceModelReadmeStatus.tooLarge,
      'unavailable' => HuggingFaceModelReadmeStatus.unavailable,
      _ => null,
    };
    if (rawFiles is! List<Object?> ||
        rawGroups is! List<Object?> ||
        readmeStatus == null ||
        (readmeStatus == HuggingFaceModelReadmeStatus.available) !=
            (json['readme'] is String) ||
        (json['readmeErrorCode'] != null &&
            json['readmeErrorCode'] is! String)) {
      throw const FormatException('Hugging Face model details were invalid.');
    }
    return HuggingFaceModelDetails(
      repoId: _requiredString(json, 'repoId'),
      revision: _requiredString(json, 'revision'),
      gated: _optionalBool(json['gated']) ?? false,
      private: _optionalBool(json['private']) ?? false,
      license: _optionalString(json['license']),
      readmeMarkdown: _optionalString(json['readme']),
      readmeStatus: readmeStatus,
      readmeErrorCode: _optionalString(json['readmeErrorCode']),
      files: rawFiles
          .map((value) => HuggingFaceModelFile.fromJson(_objectMap(value)))
          .toList(growable: false),
      downloadGroups: rawGroups
          .map((value) => HuggingFaceDownloadGroup.fromJson(_objectMap(value)))
          .toList(growable: false),
    );
  }
}

class HuggingFaceDownloadProgress {
  const HuggingFaceDownloadProgress({
    required this.fileName,
    required this.downloadedBytes,
    required this.totalBytes,
    required this.fileIndex,
    required this.fileCount,
  });

  final String fileName;
  final int downloadedBytes;
  final int totalBytes;
  final int fileIndex;
  final int fileCount;

  double? get fraction => totalBytes <= 0
      ? null
      : (downloadedBytes / totalBytes).clamp(0.0, 1.0).toDouble();

  factory HuggingFaceDownloadProgress.fromJson(Map<String, Object?> json) {
    return HuggingFaceDownloadProgress(
      fileName: _requiredString(json, 'fileName'),
      downloadedBytes: _requiredNonNegativeInt(json, 'totalDownloadedBytes'),
      totalBytes: _requiredNonNegativeInt(json, 'totalBytes'),
      fileIndex: _requiredNonNegativeInt(json, 'fileIndex'),
      fileCount: _requiredNonNegativeInt(json, 'fileCount'),
    );
  }
}

class HuggingFaceDownloadedModel {
  const HuggingFaceDownloadedModel({
    required this.repoId,
    required this.revision,
    required this.engineId,
    required this.downloadedFiles,
    required this.totalBytes,
    this.model,
    this.componentPath,
  });

  final String repoId;
  final String revision;
  final String engineId;
  final int downloadedFiles;
  final int totalBytes;
  final Map<String, Object?>? model;
  final String? componentPath;

  factory HuggingFaceDownloadedModel.fromJson(Map<String, Object?> json) {
    final rawModel = json['model'];
    final rawComponentPath = json['componentPath'];
    final componentPath = switch (rawComponentPath) {
      null => null,
      String value when value.isNotEmpty => value,
      _ => throw const FormatException(
        'Hugging Face download response contained an invalid component path.',
      ),
    };
    if (componentPath == null && rawModel == null) {
      throw const FormatException(
        'Hugging Face download response omitted the registered model.',
      );
    }
    if (rawModel != null && rawModel is! Map<String, Object?>) {
      throw const FormatException(
        'Hugging Face download response contained an invalid model.',
      );
    }
    return HuggingFaceDownloadedModel(
      repoId: _requiredString(json, 'repoId'),
      revision: _requiredString(json, 'revision'),
      engineId: _requiredString(json, 'engineId'),
      downloadedFiles: _requiredNonNegativeInt(json, 'downloadedFiles'),
      totalBytes: _requiredNonNegativeInt(json, 'totalBytes'),
      model: rawModel == null ? null : _objectMap(rawModel),
      componentPath: componentPath,
    );
  }
}

class HuggingFaceModelsRepository {
  const HuggingFaceModelsRepository(this._serviceClient);

  static const _hubRequestTimeout = Duration(seconds: 50);

  final OpenChatServiceClient _serviceClient;

  Future<HuggingFaceModelSearchPage> search({
    required String query,
    required HuggingFaceModelFormat format,
    required HuggingFaceModelSort sort,
    String? cursor,
  }) async {
    final response = await _serviceClient.call(
      'models.hub.search',
      params: <String, Object?>{
        'query': query,
        'format': format.wireValue,
        'sort': sort.wireValue,
        'cursor': cursor,
      },
      timeout: _hubRequestTimeout,
    );
    final rawModels = response['models'];
    final nextCursor = switch (response['nextCursor']) {
      null => null,
      String value when value.isNotEmpty && value.length <= 4096 => value,
      _ => throw const FormatException(
        'Hugging Face search cursor was invalid.',
      ),
    };
    if (response['freshness'] != 'current' || rawModels is! List<Object?>) {
      throw const FormatException('Hugging Face search response was invalid.');
    }
    return HuggingFaceModelSearchPage(
      models: rawModels
          .map(
            (value) => HuggingFaceModelSearchResult.fromJson(_objectMap(value)),
          )
          .toList(growable: false),
      nextCursor: nextCursor,
    );
  }

  Future<HuggingFaceModelDetails> loadDetails({
    required String repoId,
    required HuggingFaceModelFormat format,
  }) async {
    final response = await _serviceClient.call(
      'models.hub.files',
      params: <String, Object?>{'repoId': repoId, 'format': format.wireValue},
      timeout: _hubRequestTimeout,
    );
    return HuggingFaceModelDetails.fromJson(response);
  }

  Future<OpenChatServiceOperation> startDownload({
    required String repoId,
    required String revision,
    required HuggingFaceModelFormat format,
    required String groupId,
    String? componentPath,
    String? modelDirectory,
  }) {
    return _serviceClient.startOperation(
      'models.hub.download',
      params: <String, Object?>{
        'repoId': repoId,
        'revision': revision,
        'format': format.wireValue,
        'groupId': groupId,
        'componentPath': componentPath,
        'modelDirectory': modelDirectory,
      },
    );
  }
}

enum HuggingFaceDownloadStatus {
  idle,
  downloading,
  cancelling,
  completed,
  failed,
  cancelled,
}

class HuggingFaceDownloadController extends ChangeNotifier {
  HuggingFaceDownloadController(this._repository);

  final HuggingFaceModelsRepository _repository;
  HuggingFaceDownloadStatus _status = HuggingFaceDownloadStatus.idle;
  HuggingFaceDownloadProgress? _progress;
  HuggingFaceDownloadedModel? _downloadedModel;
  OpenChatServiceException? _error;
  bool _progressUnavailable = false;
  OpenChatServiceOperation? _operation;
  StreamSubscription<OpenChatServiceEvent>? _eventSubscription;
  String? _activeRepoId;

  HuggingFaceDownloadStatus get status => _status;
  HuggingFaceDownloadProgress? get progress => _progress;
  HuggingFaceDownloadedModel? get downloadedModel => _downloadedModel;
  OpenChatServiceException? get error => _error;
  bool get progressUnavailable => _progressUnavailable;
  String? get activeRepoId => _activeRepoId;
  bool get isActive =>
      _status == HuggingFaceDownloadStatus.downloading ||
      _status == HuggingFaceDownloadStatus.cancelling;

  Future<void> start({
    required String repoId,
    required String revision,
    required HuggingFaceModelFormat format,
    required String groupId,
    String? componentPath,
    String? modelDirectory,
  }) async {
    if (isActive) return;
    await _eventSubscription?.cancel();
    _eventSubscription = null;
    _operation = null;
    _progress = null;
    _downloadedModel = null;
    _error = null;
    _progressUnavailable = false;
    _activeRepoId = repoId;
    _status = HuggingFaceDownloadStatus.downloading;
    notifyListeners();

    try {
      final operation = await _repository.startDownload(
        repoId: repoId,
        revision: revision,
        format: format,
        groupId: groupId,
        componentPath: componentPath,
        modelDirectory: modelDirectory,
      );
      _operation = operation;
      _eventSubscription = operation.events.listen(
        _onEvent,
        onError: (Object error, StackTrace stackTrace) {
          _progressUnavailable = true;
          _error = const OpenChatServiceException(
            code: 'hugging_face_progress_unavailable',
            message: 'Download progress could not be read.',
          );
          notifyListeners();
        },
      );
      final result = await operation.result;
      _downloadedModel = HuggingFaceDownloadedModel.fromJson(result);
      _status = HuggingFaceDownloadStatus.completed;
    } on OpenChatServiceException catch (error) {
      _error = error;
      _status = error.code == 'operation_cancelled'
          ? HuggingFaceDownloadStatus.cancelled
          : HuggingFaceDownloadStatus.failed;
    } on FormatException {
      _error = const OpenChatServiceException(
        code: 'hugging_face_response_invalid',
        message: 'The downloaded model response was invalid.',
      );
      _status = HuggingFaceDownloadStatus.failed;
    } finally {
      await _eventSubscription?.cancel();
      _eventSubscription = null;
      _operation = null;
      notifyListeners();
    }
  }

  Future<void> cancel() async {
    if (!isActive) return;
    _status = HuggingFaceDownloadStatus.cancelling;
    notifyListeners();
    await _operation?.cancel();
  }

  void reset() {
    if (isActive) return;
    _status = HuggingFaceDownloadStatus.idle;
    _progress = null;
    _downloadedModel = null;
    _error = null;
    _progressUnavailable = false;
    notifyListeners();
  }

  void _onEvent(OpenChatServiceEvent event) {
    if (event.name != 'models.hub.download.progress') return;
    try {
      _progress = HuggingFaceDownloadProgress.fromJson(event.data);
      notifyListeners();
    } on FormatException catch (error) {
      _progressUnavailable = true;
      _error = OpenChatServiceException(
        code: 'hugging_face_progress_invalid',
        message: error.toString(),
      );
      notifyListeners();
    }
  }

  @override
  void dispose() {
    unawaited(_eventSubscription?.cancel());
    super.dispose();
  }
}

Map<String, Object?> _objectMap(Object? value) {
  if (value is! Map<String, Object?>) {
    throw const FormatException(
      'Hugging Face response contained an invalid object.',
    );
  }
  return value;
}

String _requiredString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! String || value.isEmpty) {
    throw FormatException('Hugging Face response omitted $key.');
  }
  return value;
}

String? _optionalString(Object? value) => value is String ? value : null;

bool? _optionalBool(Object? value) => value is bool ? value : null;

int _nonNegativeInt(Object? value) => _optionalNonNegativeInt(value) ?? 0;

int? _optionalNonNegativeInt(Object? value) =>
    value is int && value >= 0 ? value : null;

int _requiredNonNegativeInt(Map<String, Object?> json, String key) {
  final value = _optionalNonNegativeInt(json[key]);
  if (value == null) {
    throw FormatException('Hugging Face response omitted $key.');
  }
  return value;
}

List<String> _stringList(Object? value) {
  if (value is! List<Object?>) return const <String>[];
  return value.whereType<String>().toList(growable: false);
}
