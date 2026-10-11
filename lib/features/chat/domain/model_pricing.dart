class ModelPricingSnapshot {
  const ModelPricingSnapshot({
    required this.state,
    required this.fetchedAtUnixMs,
    required this.model,
  });

  final String state;
  final int? fetchedAtUnixMs;
  final ModelPricingDetails? model;

  factory ModelPricingSnapshot.fromJson(Map<String, Object?> json) {
    final state = json['state'];
    final fetchedAtUnixMs = json['fetchedAtUnixMs'];
    final rawModel = json['model'];
    final modelJson = _objectMap(rawModel);
    if (state is! String ||
        !const <String>{'current', 'stale', 'unavailable'}.contains(state) ||
        (fetchedAtUnixMs != null && fetchedAtUnixMs is! int) ||
        (rawModel != null && modelJson == null)) {
      throw const FormatException('Invalid model pricing response.');
    }
    return ModelPricingSnapshot(
      state: state,
      fetchedAtUnixMs: fetchedAtUnixMs is int ? fetchedAtUnixMs : null,
      model: modelJson == null ? null : ModelPricingDetails.fromJson(modelJson),
    );
  }
}

class ModelPricingDetails {
  const ModelPricingDetails({
    required this.providerId,
    required this.providerName,
    required this.modelId,
    required this.modelName,
    required this.contextWindow,
    required this.maximumOutput,
    required this.inputModalities,
    required this.outputModalities,
    required this.supportsTools,
    required this.pricing,
  });

  final String providerId;
  final String? providerName;
  final String modelId;
  final String? modelName;
  final int? contextWindow;
  final int? maximumOutput;
  final List<String> inputModalities;
  final List<String> outputModalities;
  final bool? supportsTools;
  final ModelTokenPricing? pricing;

  factory ModelPricingDetails.fromJson(Map<String, Object?> json) {
    final providerId = json['providerId'];
    final modelId = json['modelId'];
    final pricingJson = _objectMap(json['pricing']);
    final rawSupportsTools = json['supportsTools'];
    if (providerId is! String ||
        providerId.isEmpty ||
        modelId is! String ||
        modelId.isEmpty) {
      throw const FormatException('Invalid model pricing identity.');
    }
    return ModelPricingDetails(
      providerId: providerId,
      providerName: _optionalString(json['providerName']),
      modelId: modelId,
      modelName: _optionalString(json['modelName']),
      contextWindow: _optionalPositiveInt(json['contextWindow']),
      maximumOutput: _optionalPositiveInt(json['maximumOutput']),
      inputModalities: _stringList(json['inputModalities']),
      outputModalities: _stringList(json['outputModalities']),
      supportsTools: rawSupportsTools is bool ? rawSupportsTools : null,
      pricing: pricingJson == null
          ? null
          : ModelTokenPricing.fromJson(pricingJson),
    );
  }
}

class ModelTokenPricing {
  const ModelTokenPricing({
    required this.input,
    required this.output,
    required this.reasoning,
    required this.cacheRead,
    required this.cacheWrite,
    required this.tiers,
  });

  final double input;
  final double output;
  final double? reasoning;
  final double? cacheRead;
  final double? cacheWrite;
  final List<ModelTokenPricingTier> tiers;

  factory ModelTokenPricing.fromJson(Map<String, Object?> json) {
    final input = _nonNegativeNumber(json['input']);
    final output = _nonNegativeNumber(json['output']);
    if (input == null || output == null) {
      throw const FormatException('Invalid model token pricing.');
    }
    final tiers = <ModelTokenPricingTier>[];
    final rawTiers = json['tiers'];
    if (rawTiers is List) {
      for (final rawTier in rawTiers.take(32)) {
        final tier = _objectMap(rawTier);
        if (tier == null) continue;
        final parsed = ModelTokenPricingTier.tryParse(tier);
        if (parsed != null) tiers.add(parsed);
      }
    }
    return ModelTokenPricing(
      input: input,
      output: output,
      reasoning: _nonNegativeNumber(json['reasoning']),
      cacheRead: _nonNegativeNumber(json['cacheRead']),
      cacheWrite: _nonNegativeNumber(json['cacheWrite']),
      tiers: List<ModelTokenPricingTier>.unmodifiable(tiers),
    );
  }
}

class ModelTokenPricingTier {
  const ModelTokenPricingTier({
    required this.contextWindow,
    required this.input,
    required this.output,
    this.reasoning,
    this.cacheRead,
    this.cacheWrite,
  });

  final int contextWindow;
  final double input;
  final double output;
  final double? reasoning;
  final double? cacheRead;
  final double? cacheWrite;

  static ModelTokenPricingTier? tryParse(Map<String, Object?> json) {
    final contextWindow = _optionalPositiveInt(json['context']);
    final input = _nonNegativeNumber(json['input']);
    final output = _nonNegativeNumber(json['output']);
    if (contextWindow == null || input == null || output == null) return null;
    return ModelTokenPricingTier(
      contextWindow: contextWindow,
      input: input,
      output: output,
      reasoning: _nonNegativeNumber(json['reasoning']),
      cacheRead: _nonNegativeNumber(json['cacheRead']),
      cacheWrite: _nonNegativeNumber(json['cacheWrite']),
    );
  }
}

Map<String, Object?>? _objectMap(Object? value) {
  if (value is! Map) return null;
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    if (entry.key case final String key) {
      result[key] = entry.value;
    } else {
      return null;
    }
  }
  return result;
}

String? _optionalString(Object? value) =>
    value is String && value.trim().isNotEmpty ? value.trim() : null;

int? _optionalPositiveInt(Object? value) =>
    value is int && value > 0 ? value : null;

double? _nonNegativeNumber(Object? value) {
  if (value is! num) return null;
  final number = value.toDouble();
  return number.isFinite && number >= 0 ? number : null;
}

List<String> _stringList(Object? value) {
  if (value is! List) return const <String>[];
  return List<String>.unmodifiable(
    value
        .whereType<String>()
        .where((item) => item.isNotEmpty && item.length <= 64)
        .take(16),
  );
}
