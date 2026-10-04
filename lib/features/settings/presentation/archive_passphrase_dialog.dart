import 'dart:convert';

import 'package:flutter/material.dart';

import 'package:openchat/l10n/openchat_localizations.dart';

Future<String?> showArchivePassphraseDialog(
  BuildContext context, {
  bool confirmPassphrase = false,
}) {
  return showDialog<String>(
    context: context,
    builder: (context) =>
        _ArchivePassphraseDialog(confirmPassphrase: confirmPassphrase),
  );
}

String? validateArchivePassphrase(String? value, BuildContext context) {
  final l10n = context.openchatL10n;
  if (value == null || value.runes.length < 12) {
    return l10n.conversationArchivePassphraseTooShort;
  }
  if (utf8.encode(value).length > 512 || value.contains('\u0000')) {
    return l10n.conversationArchivePassphraseTooLong;
  }
  return null;
}

class _ArchivePassphraseDialog extends StatefulWidget {
  const _ArchivePassphraseDialog({required this.confirmPassphrase});

  final bool confirmPassphrase;

  @override
  State<_ArchivePassphraseDialog> createState() =>
      _ArchivePassphraseDialogState();
}

class _ArchivePassphraseDialogState extends State<_ArchivePassphraseDialog> {
  final _formKey = GlobalKey<FormState>();
  final _passphraseController = TextEditingController();
  final _confirmationController = TextEditingController();

  @override
  void dispose() {
    _passphraseController.dispose();
    _confirmationController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return AlertDialog(
      title: Text(l10n.conversationArchivePassphraseTitle),
      content: SizedBox(
        width: 420,
        child: Form(
          key: _formKey,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextFormField(
                controller: _passphraseController,
                obscureText: true,
                enableSuggestions: false,
                autocorrect: false,
                autofocus: true,
                validator: (value) => validateArchivePassphrase(value, context),
                onFieldSubmitted: (_) => _submit(context),
                decoration: InputDecoration(
                  labelText: l10n.conversationArchivePassphrase,
                  hintText: l10n.conversationArchivePassphraseHint,
                ),
              ),
              if (widget.confirmPassphrase) ...[
                const SizedBox(height: 12),
                TextFormField(
                  controller: _confirmationController,
                  obscureText: true,
                  enableSuggestions: false,
                  autocorrect: false,
                  validator: (value) {
                    final error = validateArchivePassphrase(value, context);
                    if (error != null) return error;
                    if (value != _passphraseController.text) {
                      return l10n.conversationArchivePassphraseMismatch;
                    }
                    return null;
                  },
                  onFieldSubmitted: (_) => _submit(context),
                  decoration: InputDecoration(
                    labelText: l10n.conversationArchiveConfirmPassphrase,
                  ),
                ),
                const SizedBox(height: 12),
                Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    l10n.conversationArchivePassphraseRecovery,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => _submit(context),
          child: Text(l10n.continueLabel),
        ),
      ],
    );
  }

  void _submit(BuildContext context) {
    if (!_formKey.currentState!.validate()) return;
    final passphrase = _passphraseController.text;
    _passphraseController.clear();
    _confirmationController.clear();
    Navigator.of(context).pop(passphrase);
  }
}
