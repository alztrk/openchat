import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/domain/model_catalog_freshness.dart';

void main() {
  test('selected model freshness follows its own provider route', () {
    expect(
      modelCatalogNeedsVerification(
        selectedRouteKey: 'chatgpt:account-a:workspace-a:model-1',
        availableRouteKeys: const <String>[
          'chatgpt:account-a:workspace-a:model-1',
          'chatgpt:account-b:workspace-b:model-1',
        ],
        staleRouteKeys: const <String>{'chatgpt:account-b:workspace-b:model-1'},
      ),
      isFalse,
    );
  });

  test(
    'unselected catalog needs verification only when all routes are stale',
    () {
      expect(
        modelCatalogNeedsVerification(
          selectedRouteKey: null,
          availableRouteKeys: const <String>['current', 'stale'],
          staleRouteKeys: const <String>{'stale'},
        ),
        isFalse,
      );
      expect(
        modelCatalogNeedsVerification(
          selectedRouteKey: null,
          availableRouteKeys: const <String>['stale-a', 'stale-b'],
          staleRouteKeys: const <String>{'stale-a', 'stale-b'},
        ),
        isTrue,
      );
      expect(
        modelCatalogNeedsVerification(
          selectedRouteKey: null,
          availableRouteKeys: const <String>[],
          staleRouteKeys: const <String>{},
        ),
        isFalse,
      );
    },
  );
}
