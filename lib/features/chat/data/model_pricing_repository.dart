import 'package:openchat/features/chat/domain/model_pricing.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ModelPricingRepository {
  const ModelPricingRepository(this._serviceClient);

  final OpenChatServiceClient _serviceClient;

  Future<ModelPricingSnapshot> load({
    required String providerId,
    required String modelId,
  }) async {
    final response = await _serviceClient.call(
      'models.pricing.get',
      params: <String, Object?>{'providerId': providerId, 'modelId': modelId},
    );
    return ModelPricingSnapshot.fromJson(response);
  }
}
