import 'dart:async';

import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';
import 'package:openchat/features/settings/data/open_code_api_key_store.dart';

class OpenCodeConnectionSection extends StatefulWidget {
  const OpenCodeConnectionSection({
    super.key,
    required this.apiKeyStore,
    required this.onChanged,
  });

  final OpenCodeApiKeyStore? apiKeyStore;
  final Future<void> Function()? onChanged;

  @override
  State<OpenCodeConnectionSection> createState() =>
      _OpenCodeConnectionSectionState();
}

class _OpenCodeConnectionSectionState extends State<OpenCodeConnectionSection> {
  final _controller = TextEditingController();
  String? _keySuffix;
  bool _loading = true;
  bool _saving = false;
  bool _showForm = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    final store = widget.apiKeyStore;
    if (store == null) {
      if (mounted) setState(() => _loading = false);
      return;
    }
    try {
      final suffix = await store.readKeySuffix();
      if (mounted) {
        setState(() {
          _keySuffix = suffix;
          _loading = false;
        });
      }
    } on OpenCodeApiKeyStorageException {
      if (mounted) {
        setState(() {
          _error = 'storage';
          _loading = false;
        });
      }
    }
  }

  Future<void> _save() async {
    final store = widget.apiKeyStore;
    if (store == null || _saving) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await store.saveApiKey(_controller.text);
      _controller.clear();
      await _load();
      await widget.onChanged?.call();
      if (mounted) setState(() => _showForm = false);
    } on InvalidOpenCodeApiKeyException {
      if (mounted) setState(() => _error = 'invalid');
    } on OpenCodeApiKeyStorageException {
      if (mounted) setState(() => _error = 'storage');
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _remove() async {
    final store = widget.apiKeyStore;
    if (store == null || _saving) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await store.deleteApiKey();
      await _load();
      await widget.onChanged?.call();
    } on OpenCodeApiKeyStorageException {
      if (mounted) setState(() => _error = 'storage');
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final hasKey = _keySuffix != null;
    final actions = Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        OutlinedButton.icon(
          onPressed: _saving
              ? null
              : () => setState(() => _showForm = !_showForm),
          icon: Icon(
            hasKey ? Icons.edit_outlined : Icons.add_rounded,
            size: 16,
          ),
          label: Text(hasKey ? l10n.edit : l10n.add),
        ),
        if (hasKey)
          TextButton.icon(
            onPressed: _saving ? null : _remove,
            icon: const Icon(Icons.delete_outline_rounded, size: 16),
            label: Text(l10n.deleteAll),
          ),
      ],
    );
    return Card(
      margin: EdgeInsets.zero,
      color: palette.surface,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
        side: BorderSide(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                ProviderIcon(
                  providerId: 'opencode',
                  color: palette.text,
                  size: 24,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        l10n.openCodeConsole,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 4),
                      Text(
                        l10n.openCodeConsoleDescription,
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 13,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
            if (_error == 'storage' && !_showForm) ...[
              const SizedBox(height: 10),
              Text(
                l10n.openCodeKeyStorageFailed,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
            const SizedBox(height: 12),
            if (_loading) const LinearProgressIndicator(),
            if (!_loading)
              LayoutBuilder(
                builder: (context, constraints) {
                  final status = Text(
                    hasKey
                        ? l10n.openCodeKeySaved(_keySuffix!)
                        : l10n.openCodeNoKey,
                  );
                  if (constraints.maxWidth /
                          MediaQuery.textScalerOf(context).scale(1) <
                      520) {
                    return Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        status,
                        const SizedBox(height: 8),
                        Align(alignment: Alignment.centerRight, child: actions),
                      ],
                    );
                  }
                  return Row(
                    children: [
                      Expanded(child: status),
                      const SizedBox(width: 12),
                      actions,
                    ],
                  );
                },
              ),
            if (_showForm)
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  TextField(
                    controller: _controller,
                    obscureText: true,
                    autocorrect: false,
                    enableSuggestions: false,
                    decoration: InputDecoration(
                      labelText: l10n.openCodeApiKey,
                      errorText: _error == 'invalid'
                          ? l10n.openCodeKeyInvalid
                          : _error == 'storage'
                          ? l10n.openCodeKeyStorageFailed
                          : null,
                    ),
                    onSubmitted: (_) => unawaited(_save()),
                  ),
                  const SizedBox(height: 12),
                  Align(
                    alignment: Alignment.centerRight,
                    child: FilledButton(
                      onPressed: _saving ? null : _save,
                      child: Text(_saving ? l10n.saving : l10n.save),
                    ),
                  ),
                ],
              ),
          ],
        ),
      ),
    );
  }
}
