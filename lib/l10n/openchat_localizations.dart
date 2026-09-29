import 'package:flutter/widgets.dart';

import 'package:openchat/l10n/generated/app_localizations.dart';

extension OpenChatLocalizationsX on BuildContext {
  AppLocalizations get openchatL10n {
    final localizations = AppLocalizations.of(this);
    if (localizations == null) {
      throw StateError(
        'OpenChat localizations are unavailable. Check the app delegates and supported locales.',
      );
    }
    return localizations;
  }
}
