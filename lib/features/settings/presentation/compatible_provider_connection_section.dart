import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class CompatibleProviderConnectionSection extends StatefulWidget {
  const CompatibleProviderConnectionSection({
    required this.providerId,
    required this.apiKeyStore,
    required this.onChanged,
    super.key,
  });

  final String providerId;
  final ApiCompatibleProviderKeyStore? apiKeyStore;
  final Future<void> Function()? onChanged;

  @override
  State<CompatibleProviderConnectionSection> createState() =>
      _CompatibleProviderConnectionSectionState();
}

class _CompatibleProviderConnectionSectionState
    extends State<CompatibleProviderConnectionSection> {
  final _controller = TextEditingController();
  ApiCompatibleProviderKeyStatus? _status;
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
      final status = await store.readStatus(widget.providerId);
      if (mounted) {
        setState(() {
          _status = status;
          _loading = false;
          _error = null;
        });
      }
    } on ApiCompatibleProviderKeyStorageException {
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
      await store.saveApiKey(widget.providerId, _controller.text);
      _controller.clear();
      await _load();
      await widget.onChanged?.call();
      if (mounted) setState(() => _showForm = false);
    } on InvalidApiCompatibleProviderKeyException {
      if (mounted) setState(() => _error = 'invalid');
    } on ApiCompatibleProviderKeyStorageException {
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
      await store.deleteApiKey(widget.providerId);
      await _load();
      await widget.onChanged?.call();
    } on ApiCompatibleProviderKeyStorageException {
      if (mounted) setState(() => _error = 'storage');
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final configured = _status?.isConfigured == true;
    final name = _name(l10n);
    final description = _description(l10n);
    final actions = Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        OutlinedButton.icon(
          onPressed: _saving
              ? null
              : () => setState(() => _showForm = !_showForm),
          icon: Icon(
            configured ? LucideIcons.pencil : LucideIcons.plus,
            size: 16,
          ),
          label: Text(configured ? l10n.edit : l10n.add),
        ),
        if (configured)
          TextButton.icon(
            onPressed: _saving ? null : _remove,
            icon: const Icon(LucideIcons.trash2, size: 16),
            label: Text(l10n.deleteAll),
          ),
      ],
    );

    return Card(
      margin: EdgeInsets.zero,
      color: palette.surface,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(OpenChatRadii.card),
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
                  providerId: widget.providerId,
                  color: palette.text,
                  size: 24,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        name,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 4),
                      Text(
                        description,
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
            if (widget.providerId == 'gemini') ...[
              const SizedBox(height: 12),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(
                    LucideIcons.info,
                    size: 16,
                    color: palette.secondaryIcon,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      l10n.geminiUnpaidDataNotice,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                        height: 1.4,
                      ),
                    ),
                  ),
                ],
              ),
            ],
            if (_error == 'storage' && !_showForm) ...[
              const SizedBox(height: 10),
              Text(
                l10n.providerKeyStorageFailed,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
            const SizedBox(height: 12),
            if (_loading)
              const LinearProgressIndicator()
            else
              LayoutBuilder(
                builder: (context, constraints) {
                  final suffix = _status?.keySuffix;
                  final status = Text(
                    configured
                        ? suffix == null
                              ? l10n.providerKeySaved
                              : l10n.providerKeySavedSuffix(suffix)
                        : l10n.providerNoKey,
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
            if (_showForm) ...[
              const SizedBox(height: 12),
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  TextField(
                    controller: _controller,
                    obscureText: true,
                    autocorrect: false,
                    enableSuggestions: false,
                    decoration: InputDecoration(
                      labelText: l10n.providerApiKey,
                      errorText: _error == 'invalid'
                          ? l10n.providerKeyInvalid
                          : _error == 'storage'
                          ? l10n.providerKeyStorageFailed
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
          ],
        ),
      ),
    );
  }

  String _name(AppLocalizations l10n) => switch (widget.providerId) {
    'gemini' => l10n.geminiProvider,
    'groq' => l10n.groqProvider,
    'cerebras' => l10n.cerebrasProvider,
    'openrouter' => l10n.openRouterProvider,
    'mistral' => l10n.mistralProvider,
    _ => widget.providerId,
  };

  String _description(AppLocalizations l10n) => switch (widget.providerId) {
    'gemini' => l10n.geminiApiDescription,
    'groq' => l10n.groqApiDescription,
    'cerebras' => l10n.cerebrasApiDescription,
    'openrouter' => l10n.openRouterApiDescription,
    'mistral' => l10n.mistralApiDescription,
    _ => '',
  };
}
