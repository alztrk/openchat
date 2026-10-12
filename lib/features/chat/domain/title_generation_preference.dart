import 'default_model_preference.dart';

enum TitleGenerationMode { conversation, selectedModel, lightweight }

class TitleGenerationPreference {
  const TitleGenerationPreference({
    this.mode = TitleGenerationMode.conversation,
    this.model,
  });

  final TitleGenerationMode mode;
  final DefaultModelPreference? model;

  Map<String, Object?> toJson() => <String, Object?>{
    'mode': mode.name,
    if (model != null) 'model': model!.toJson(),
  };

  factory TitleGenerationPreference.fromJson(Map<String, Object?> json) {
    final mode = switch (json['mode']) {
      'conversation' => TitleGenerationMode.conversation,
      'selectedModel' => TitleGenerationMode.selectedModel,
      'lightweight' => TitleGenerationMode.lightweight,
      _ => throw const FormatException('The title generation mode is invalid.'),
    };
    final rawModel = json['model'];
    DefaultModelPreference? model;
    if (rawModel != null) {
      if (rawModel is! Map) {
        throw const FormatException('The title generation model is invalid.');
      }
      final values = <String, Object?>{};
      for (final entry in rawModel.entries) {
        if (entry.key is! String) {
          throw const FormatException('The title generation model is invalid.');
        }
        values[entry.key as String] = entry.value;
      }
      model = DefaultModelPreference.fromJson(values);
    }
    if ((mode == TitleGenerationMode.conversation && model != null) ||
        (mode == TitleGenerationMode.selectedModel && model == null)) {
      throw const FormatException('The title generation model is invalid.');
    }
    return TitleGenerationPreference(mode: mode, model: model);
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is TitleGenerationPreference &&
          mode == other.mode &&
          model == other.model;

  @override
  int get hashCode => Object.hash(mode, model);
}
