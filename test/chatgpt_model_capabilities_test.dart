import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';

void main() {
  test('parses nullable tool capability without inventing support', () {
    final model = ChatGptModel.fromJson(<String, Object?>{
      'id': 'mistral-small-latest',
      'displayName': 'Mistral Small',
      'reasoningLevels': <String>[],
      'supportsTools': false,
      'isAvailable': true,
    });

    expect(model.supportsTools, isFalse);
    expect(model.withRoute(providerId: 'mistral').supportsTools, isFalse);
    expect(model.withRoute(providerId: 'chatgpt').supportsTools, isTrue);

    final unknownModel = ChatGptModel.fromJson(<String, Object?>{
      'id': 'unlisted-model',
      'displayName': 'Unlisted model',
      'reasoningLevels': <String>[],
      'isAvailable': true,
    });
    expect(unknownModel.withRoute(providerId: 'groq').supportsTools, isNull);
  });

  test('rejects malformed tool capability metadata', () {
    expect(
      () => ChatGptModel.fromJson(<String, Object?>{
        'id': 'invalid-model',
        'displayName': 'Invalid model',
        'reasoningLevels': <String>[],
        'supportsTools': 'yes',
        'isAvailable': true,
      }),
      throwsFormatException,
    );
  });
}
