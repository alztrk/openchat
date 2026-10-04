import 'local_engines_models.dart';

import 'package:openchat/platform/windows/openchat_service_client.dart';

enum LocalModelStorageAction {
  move('move'),
  copy('copy'),
  keep('keep');

  const LocalModelStorageAction(this.wireValue);

  final String wireValue;
}

class LocalEnginesRepository {
  const LocalEnginesRepository(this._serviceClient);

  final OpenChatServiceClient _serviceClient;

  Future<LocalEngineCatalog> loadCatalog() async {
    final response = await _serviceClient.call('local.engines.list');
    return LocalEngineCatalog.fromJson(response);
  }

  Future<void> setLlamaServerExecutablePath(String? path) async {
    final response = await _serviceClient.call(
      'local.engines.llama_server_path.set',
      params: <String, Object?>{'path': path},
    );
    if (response['available'] != (path != null)) {
      throw const FormatException(
        'The llama-server executable setting was not saved.',
      );
    }
  }

  Future<List<LocalExternalLlamaServerCandidate>>
  detectExternalLlamaServers() async {
    final response = await _serviceClient.call(
      'local.engines.external_llama_server.detect',
    );
    final rawServers = response['servers'];
    if (rawServers is! List<Object?>) {
      throw const FormatException(
        'The external llama-server scan response was invalid.',
      );
    }
    return rawServers
        .map(
          (server) => LocalExternalLlamaServerCandidate.fromJson(
            _objectMap(server),
          ),
        )
        .toList(growable: false);
  }

  Future<LocalExternalLlamaServerState> connectExternalLlamaServer(
    LocalExternalLlamaServerCandidate candidate,
  ) async {
    final response = await _serviceClient.call(
      'local.engines.external_llama_server.connect',
      params: <String, Object?>{
        'processId': candidate.processId,
        'port': candidate.port,
      },
    );
    return LocalExternalLlamaServerState.fromJson(response);
  }

  Future<void> disconnectExternalLlamaServer() async {
    final response = await _serviceClient.call(
      'local.engines.external_llama_server.disconnect',
    );
    if (response['disconnected'] != true) {
      throw const FormatException(
        'The external llama-server connection was not cleared.',
      );
    }
  }

  Future<LocalModelCatalog> loadModels() async {
    final response = await _serviceClient.call('local.models.list');
    return LocalModelCatalog.fromJson(response);
  }

  Future<LocalModelDiscovery> discoverModels({
    required String engineId,
    required String modelDirectory,
  }) async {
    final response = await _serviceClient.call(
      'local.models.discover',
      params: <String, Object?>{
        'engineId': engineId,
        'modelDirectory': modelDirectory,
      },
    );
    return LocalModelDiscovery.fromJson(response);
  }

  Future<LocalRegisteredModel> registerModel({
    required String engineId,
    required String modelPath,
    required String modelDirectory,
    required LocalModelStorageAction storageAction,
  }) async {
    final operation = await _serviceClient.startOperation(
      'local.models.register',
      params: <String, Object?>{
        'engineId': engineId,
        'modelPath': modelPath,
        'modelDirectory': modelDirectory,
        'storageAction': storageAction.wireValue,
      },
    );
    final response = await operation.result;
    return LocalRegisteredModel.fromJson(response);
  }

  Future<bool> removeModel(String modelId) async {
    final response = await _serviceClient.call(
      'local.models.remove',
      params: <String, Object?>{'modelId': modelId},
    );
    final removed = response['removed'];
    if (removed is! bool) {
      throw const FormatException('Local model removal response was invalid.');
    }
    return removed;
  }

  Future<LocalEngineRuntimeOperation> startModel(String modelId) async {
    final operation = await _serviceClient.startOperation(
      'local.engines.start',
      params: <String, Object?>{'modelId': modelId},
    );
    return LocalEngineRuntimeOperation.fromServiceOperation(operation);
  }

  Future<LocalEngineRuntimeState> stopRuntime() async {
    final response = await _serviceClient.call('local.engines.stop');
    return LocalEngineRuntimeState.fromJson(response);
  }

  Future<LocalEngineInstallOperation> install({
    required String engineId,
    required String variantId,
  }) async {
    final operation = await _serviceClient.startOperation(
      'local.engines.install',
      params: <String, Object?>{'engineId': engineId, 'variantId': variantId},
    );
    return LocalEngineInstallOperation.fromServiceOperation(operation);
  }
}

Map<String, Object?> _objectMap(Object? value) {
  if (value is Map<String, Object?>) return value;
  if (value is Map) {
    final result = <String, Object?>{};
    for (final entry in value.entries) {
      if (entry.key is! String) {
        throw const FormatException(
          'The external llama-server response contained an invalid key.',
        );
      }
      result[entry.key as String] = entry.value;
    }
    return result;
  }
  throw const FormatException(
    'The external llama-server response was invalid.',
  );
}

class LocalEngineRuntimeOperation {
  const LocalEngineRuntimeOperation.fromServiceOperation(this._operation);

  final OpenChatServiceOperation _operation;

  Future<LocalEngineRuntimeState> get result async =>
      LocalEngineRuntimeState.fromJson(await _operation.result);

  Future<bool> cancel() => _operation.cancel();
}
