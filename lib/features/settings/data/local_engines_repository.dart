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

  Future<LocalModelCatalog> loadModels() async {
    final response = await _serviceClient.call('local.models.list');
    return LocalModelCatalog.fromJson(response);
  }

  Future<LocalRegisteredModel> registerModel({
    required String engineId,
    required String modelPath,
    required LocalModelStorageAction storageAction,
  }) async {
    final operation = await _serviceClient.startOperation(
      'local.models.register',
      params: <String, Object?>{
        'engineId': engineId,
        'modelPath': modelPath,
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

class LocalEngineRuntimeOperation {
  const LocalEngineRuntimeOperation.fromServiceOperation(this._operation);

  final OpenChatServiceOperation _operation;

  Future<LocalEngineRuntimeState> get result async =>
      LocalEngineRuntimeState.fromJson(await _operation.result);

  Future<bool> cancel() => _operation.cancel();
}
