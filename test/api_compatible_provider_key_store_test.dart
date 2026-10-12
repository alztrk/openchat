import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('providerIds includes all expected pre-configured providers', () {
    expect(ApiCompatibleProviderKeyStore.providerIds, containsAll(<String>[
      'gemini',
      'groq',
      'cerebras',
      'openrouter',
      'mistral',
      'deepseek',
      'xai',
      'perplexity',
      'together',
      'sambanova',
      'anthropic',
      'fireworks',
      'deepinfra',
      'dashscope',
      'cohere',
      'ollama',
      'lmstudio',
    ]));
    expect(ApiCompatibleProviderKeyStore.providerIds.length, equals(17));
  });

  test('localizations provide valid display names and descriptions for all providers', () async {
    for (final locale in [
      const Locale('en'),
      const Locale('tr'),
      const Locale('de'),
      const Locale('es'),
      const Locale('fr'),
    ]) {
      final l10n = await AppLocalizations.delegate.load(locale);
      expect(l10n.deepSeekProvider, isNotEmpty);
      expect(l10n.xAiProvider, isNotEmpty);
      expect(l10n.perplexityProvider, isNotEmpty);
      expect(l10n.togetherProvider, isNotEmpty);
      expect(l10n.sambaNovaProvider, isNotEmpty);
      expect(l10n.anthropicProvider, isNotEmpty);
      expect(l10n.fireworksProvider, isNotEmpty);
      expect(l10n.deepInfraProvider, isNotEmpty);
      expect(l10n.dashScopeProvider, isNotEmpty);
      expect(l10n.cohereProvider, isNotEmpty);
      expect(l10n.ollamaProvider, isNotEmpty);
      expect(l10n.lmStudioProvider, isNotEmpty);

      expect(l10n.deepSeekApiDescription, isNotEmpty);
      expect(l10n.xAiApiDescription, isNotEmpty);
      expect(l10n.perplexityApiDescription, isNotEmpty);
      expect(l10n.togetherApiDescription, isNotEmpty);
      expect(l10n.sambaNovaApiDescription, isNotEmpty);
      expect(l10n.anthropicApiDescription, isNotEmpty);
      expect(l10n.fireworksApiDescription, isNotEmpty);
      expect(l10n.deepInfraApiDescription, isNotEmpty);
      expect(l10n.dashScopeApiDescription, isNotEmpty);
      expect(l10n.cohereApiDescription, isNotEmpty);
      expect(l10n.ollamaApiDescription, isNotEmpty);
      expect(l10n.lmStudioApiDescription, isNotEmpty);
    }
  });

  testWidgets('renders ProviderIcon for all pre-configured providers', (tester) async {
    for (final providerId in ApiCompatibleProviderKeyStore.providerIds) {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ProviderIcon(
              providerId: providerId,
              color: Colors.black,
              size: 24,
            ),
          ),
        ),
      );
      expect(find.byType(ProviderIcon), findsOneWidget);
    }
  });

  test('ApiCompatibleProviderKeyStore saves, reads status, and clears keys', () async {
    FlutterSecureStorage.setMockInitialValues({});
    const secureStorage = FlutterSecureStorage();
    final store = ApiCompatibleProviderKeyStore(secureStorage);

    final statusBefore = await store.readStatus('deepseek');
    expect(statusBefore.isConfigured, isFalse);
    expect(statusBefore.keySuffix, isNull);

    await store.saveApiKey('deepseek', 'sk-test-deepseek-123456');

    final key = await store.readApiKey('deepseek');
    expect(key, 'sk-test-deepseek-123456');

    final statusAfter = await store.readStatus('deepseek');
    expect(statusAfter.isConfigured, isTrue);
    expect(statusAfter.keySuffix, '3456');

    final configuredIds = await store.readConfiguredProviderIds();
    expect(configuredIds, contains('deepseek'));

    await store.deleteApiKey('deepseek');
    final statusDeleted = await store.readStatus('deepseek');
    expect(statusDeleted.isConfigured, isFalse);
  });
}
