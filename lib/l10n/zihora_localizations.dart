import 'package:flutter/widgets.dart';

import 'generated/app_localizations.dart';

extension ZihoraLocalizationsX on BuildContext {
  AppLocalizations get zihoraL10n {
    final localizations = AppLocalizations.of(this);
    if (localizations == null) {
      throw StateError(
        'Zihora localizations are unavailable. Check the app delegates and supported locales.',
      );
    }
    return localizations;
  }
}
