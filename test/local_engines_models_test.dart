import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/settings/data/local_engines_models.dart';

void main() {
  group('LocalEngineCatalog', () {
    test('parses catalog metadata and installable variants', () {
      final catalog = LocalEngineCatalog.fromJson(<String, Object?>{
        'llamaServerExecutablePath': r'C:\llama\llama-server.exe',
        'llamaServerExecutableAvailable': true,
        'engines': <Object?>[
          <String, Object?>{
            'engineId': 'llama_cpp',
            'displayName': 'llama.cpp',
            'releaseTag': 'b11349',
            'channel': 'preview',
            'catalogStatus': 'installable',
            'statusReason': null,
            'runtimeStatus': 'stopped',
            'modelDirectory': r'C:\OpenChat\models\llama',
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
        'externalLlamaServer': <String, Object?>{'connected': false},
      });

      expect(catalog.engines, hasLength(1));
      expect(catalog.engines.single.engineId, 'llama_cpp');
      expect(catalog.engines.single.variants.single.canInstall, isTrue);
      expect(catalog.engines.single.variants.single.accelerator, 'cpu');
      expect(
        catalog.llamaServerExecutablePath,
        r'C:\llama\llama-server.exe',
      );
      expect(catalog.llamaServerExecutableAvailable, isTrue);
      expect(catalog.externalLlamaServer.connected, isFalse);
    });

    test('rejects an invalid configured llama-server status', () {
      expect(
        () => LocalEngineCatalog.fromJson(<String, Object?>{
          'engines': <Object?>[],
          'models': <Object?>[],
          'runtime': <String, Object?>{'status': 'stopped'},
          'externalLlamaServer': <String, Object?>{'connected': false},
          'llamaServerExecutableAvailable': 'yes',
        }),
        throwsFormatException,
      );
    });

    test('parses a validated external llama-server connection', () {
      final state = LocalExternalLlamaServerState.fromJson(
        <String, Object?>{
          'connected': true,
          'processId': 4216,
          'port': 8080,
          'modelIds': <Object?>['qwen3-8b'],
        },
      );

      expect(state.connected, isTrue);
      expect(state.processId, 4216);
      expect(state.port, 8080);
      expect(state.modelIds, <String>['qwen3-8b']);
    });

    test('rejects invalid external llama-server endpoints', () {
      expect(
        () => LocalExternalLlamaServerCandidate.fromJson(<String, Object?>{
          'processId': -1,
          'port': 70000,
        }),
        throwsFormatException,
      );
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

    test('parses discovered local models and bounded-scan status', () {
      final discovery = LocalModelDiscovery.fromJson(<String, Object?>{
        'truncated': true,
        'models': <Object?>[
          <String, Object?>{
            'engineId': 'llama_cpp',
            'displayName': 'Qwen 7B',
            'path': r'C:\models\qwen-7b.gguf',
            'pathKind': 'file',
          },
        ],
      });

      expect(discovery.models, hasLength(1));
      expect(discovery.models.single.key, r'llama_cpp:C:\models\qwen-7b.gguf');
      expect(discovery.truncated, isTrue);
    });

    test('rejects discovered models with an unknown engine', () {
      expect(
        () => LocalDiscoveredModel.fromJson(<String, Object?>{
          'engineId': 'unknown',
          'displayName': 'Model',
          'path': r'C:\models\model.gguf',
          'pathKind': 'file',
        }),
        throwsFormatException,
      );
    });
  });
}
