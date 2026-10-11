import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/domain/model_pricing.dart';

void main() {
  test('parses catalog limits, capabilities, prices, and context tiers', () {
    final snapshot = ModelPricingSnapshot.fromJson(<String, Object?>{
      'state': 'current',
      'fetchedAtUnixMs': 1,
      'model': <String, Object?>{
        'providerId': 'openai',
        'providerName': 'OpenAI',
        'modelId': 'example',
        'modelName': 'Example',
        'contextWindow': 128000,
        'maximumOutput': 16000,
        'inputModalities': <String>['text', 'image'],
        'outputModalities': <String>['text'],
        'supportsTools': true,
        'pricing': <String, Object?>{
          'input': 2.5,
          'output': 10,
          'reasoning': 12,
          'cacheRead': 1,
          'cacheWrite': null,
          'tiers': <Object?>[
            <String, Object?>{'context': 200000, 'input': 5, 'output': 15},
          ],
        },
      },
    });

    expect(snapshot.model?.providerName, 'OpenAI');
    expect(snapshot.model?.contextWindow, 128000);
    expect(snapshot.model?.maximumOutput, 16000);
    expect(snapshot.model?.supportsTools, isTrue);
    expect(snapshot.model?.pricing?.input, 2.5);
    expect(snapshot.model?.pricing?.cacheRead, 1);
    expect(snapshot.model?.pricing?.tiers.single.contextWindow, 200000);
  });

  test('rejects an invalid pricing response state', () {
    expect(
      () => ModelPricingSnapshot.fromJson(<String, Object?>{
        'state': 'failed',
        'fetchedAtUnixMs': null,
        'model': null,
      }),
      throwsFormatException,
    );
  });
}
