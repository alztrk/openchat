import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/settings/data/local_engines_models.dart';

void main() {
  group('LocalEngineCatalog', () {
    test('parses catalog metadata and installable variants', () {
      final catalog = LocalEngineCatalog.fromJson(<String, Object?>{
        'engines': <Object?>[
          <String, Object?>{
            'engineId': 'llama_cpp',
            'displayName': 'llama.cpp',
            'releaseTag': 'b11349',
            'channel': 'preview',
            'catalogStatus': 'installable',
            'statusReason': null,
            'runtimeStatus': 'stopped',
            'variants': <Object?>[
              <String, Object?>{
                'variantId': 'win-x86_64-cpu',
                'os': 'windows',
                'architecture': 'x86_64',
                'accelerator': 'cpu',
                'runtimeRequirements': <Object?>['Windows x86_64'],
                'canInstall': true,
                'recommended': true,
                'installed': false,
                'status': 'available',
              },
            ],
          },
        ],
        'models': <Object?>[],
        'runtime': <String, Object?>{'status': 'stopped'},
      });

      expect(catalog.engines, hasLength(1));
      expect(catalog.engines.single.engineId, 'llama_cpp');
      expect(catalog.engines.single.variants.single.canInstall, isTrue);
      expect(catalog.engines.single.variants.single.accelerator, 'cpu');
    });

    test('reads native phase progress and calculates bounded fraction', () {
      final progress = LocalEngineInstallProgress.fromJson(<String, Object?>{
        'phase': 'downloading',
        'engineId': 'llama_cpp',
        'variantId': 'win-x86_64-cpu',
        'assetName': 'llama.zip',
        'assetIndex': 0,
        'assetCount': 1,
        'downloadedBytes': 25,
        'totalBytes': 100,
      });

      expect(progress.stage, 'downloading');
      expect(progress.fraction, 0.25);
    });

    test('rejects incomplete native install progress', () {
      expect(
        () => LocalEngineInstallProgress.fromJson(<String, Object?>{
          'phase': 'downloading',
        }),
        throwsFormatException,
      );
    });
  });
}
