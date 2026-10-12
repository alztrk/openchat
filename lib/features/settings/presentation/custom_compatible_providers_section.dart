import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class CustomCompatibleProvidersSection extends StatefulWidget {
  const CustomCompatibleProvidersSection({
    required this.store,
    required this.onChanged,
    super.key,
  });

  final ApiCompatibleProviderKeyStore? store;
  final Future<void> Function()? onChanged;

  @override
  State<CustomCompatibleProvidersSection> createState() =>
      _CustomCompatibleProvidersSectionState();
}

class _CustomCompatibleProvidersSectionState
    extends State<CustomCompatibleProvidersSection> {
  List<CustomCompatibleProvider> _providers = const [];
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    final store = widget.store;
    if (store == null) {
      if (mounted) setState(() => _loading = false);
      return;
    }
    try {
      final providers = await store.readCustomProviders();
      if (!mounted) return;
      setState(() {
        _providers = providers;
        _loading = false;
        _error = null;
      });
    } on ApiCompatibleProviderKeyStorageException {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = 'storage';
      });
    }
  }

  Future<void> _edit([CustomCompatibleProvider? provider]) async {
    final result = await showDialog<_CustomProviderDraft>(
      context: context,
      builder: (context) => _CustomProviderDialog(provider: provider),
    );
    if (result == null || !mounted) return;
    final store = widget.store;
    if (store == null) return;
    try {
      await store.saveCustomProvider(
        id: provider?.id,
        name: result.name,
        baseUrl: result.baseUrl,
        protocol: result.protocol,
        apiKey: result.apiKey,
      );
      await _load();
      await widget.onChanged?.call();
    } on InvalidApiCompatibleProviderKeyException {
      if (mounted) setState(() => _error = 'invalid');
    } on ApiCompatibleProviderKeyStorageException {
      if (mounted) setState(() => _error = 'storage');
    }
  }

  Future<void> _delete(CustomCompatibleProvider provider) async {
    final store = widget.store;
    if (store == null) return;
    try {
      await store.deleteCustomProvider(provider.id);
      await _load();
      await widget.onChanged?.call();
    } on ApiCompatibleProviderKeyStorageException {
      if (mounted) setState(() => _error = 'storage');
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                l10n.customProvidersHeading,
                style: Theme.of(context).textTheme.titleMedium,
              ),
            ),
            OutlinedButton.icon(
              onPressed: _loading ? null : () => unawaited(_edit()),
              icon: const Icon(LucideIcons.plus, size: 16),
              label: Text(l10n.add),
            ),
          ],
        ),
        const SizedBox(height: 6),
        Text(
          l10n.customProvidersDescription,
          style: TextStyle(color: palette.secondaryText, fontSize: 13),
        ),
        if (_error != null) ...[
          const SizedBox(height: 8),
          Text(
            _error == 'storage'
                ? l10n.providerKeyStorageFailed
                : l10n.customProviderInvalid,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
        ],
        const SizedBox(height: 10),
        if (_loading)
          const LinearProgressIndicator()
        else if (_providers.isEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 10),
            child: Text(
              l10n.customProvidersEmpty,
              style: TextStyle(color: palette.secondaryText),
            ),
          )
        else
          for (final provider in _providers)
            Card(
              margin: const EdgeInsets.only(top: 8),
              color: palette.surface,
              elevation: 0,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(OpenChatRadii.card),
                side: BorderSide(color: palette.border),
              ),
              child: ListTile(
                leading: Icon(
                  LucideIcons.network,
                  color: palette.secondaryIcon,
                ),
                title: Text(provider.name),
                subtitle: Text(
                  '${provider.protocol == CustomProviderProtocol.openAi ? l10n.openAiCompatible : l10n.anthropicCompatible} · ${provider.baseUrl}',
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                ),
                trailing: Wrap(
                  spacing: 4,
                  children: [
                    IconButton(
                      tooltip: l10n.edit,
                      onPressed: () => unawaited(_edit(provider)),
                      icon: const Icon(LucideIcons.pencil, size: 17),
                    ),
                    IconButton(
                      tooltip: l10n.deleteAll,
                      onPressed: () => unawaited(_delete(provider)),
                      icon: const Icon(LucideIcons.trash2, size: 17),
                    ),
                  ],
                ),
              ),
            ),
      ],
    );
  }
}

class _CustomProviderDraft {
  const _CustomProviderDraft({
    required this.name,
    required this.baseUrl,
    required this.protocol,
    required this.apiKey,
  });

  final String name;
  final String baseUrl;
  final CustomProviderProtocol protocol;
  final String apiKey;
}

class _CustomProviderDialog extends StatefulWidget {
  const _CustomProviderDialog({this.provider});

  final CustomCompatibleProvider? provider;

  @override
  State<_CustomProviderDialog> createState() => _CustomProviderDialogState();
}

class _CustomProviderDialogState extends State<_CustomProviderDialog> {
  late final TextEditingController _nameController;
  late final TextEditingController _urlController;
  final TextEditingController _keyController = TextEditingController();
  late CustomProviderProtocol _protocol;
  String? _error;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.provider?.name);
    _urlController = TextEditingController(text: widget.provider?.baseUrl);
    _protocol = widget.provider?.protocol ?? CustomProviderProtocol.openAi;
  }

  @override
  void dispose() {
    _nameController.dispose();
    _urlController.dispose();
    _keyController.dispose();
    super.dispose();
  }

  void _save() {
    final name = _nameController.text.trim();
    final baseUrl = _urlController.text.trim();
    final apiKey = _keyController.text.trim();
    final l10n = context.openchatL10n;
    if (name.isEmpty || name.length > 80) {
      setState(() => _error = l10n.customProviderInvalid);
      return;
    }
    final uri = Uri.tryParse(baseUrl);
    if (uri == null ||
        !const <String>{'http', 'https'}.contains(uri.scheme) ||
        uri.host.isEmpty ||
        uri.userInfo.isNotEmpty ||
        uri.hasQuery ||
        uri.hasFragment) {
      setState(() => _error = l10n.customProviderUrlInvalid);
      return;
    }
    Navigator.of(context).pop(
      _CustomProviderDraft(
        name: name,
        baseUrl: baseUrl,
        protocol: _protocol,
        apiKey: apiKey,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return Dialog(
      insetPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 24),
      child: SizedBox(
        width: 520,
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                widget.provider == null
                    ? l10n.addCustomProvider
                    : l10n.editCustomProvider,
                style: Theme.of(context).textTheme.titleLarge,
              ),
              const SizedBox(height: 24),
              TextField(
                controller: _nameController,
                autofocus: true,
                maxLength: 80,
                textCapitalization: TextCapitalization.words,
                decoration: InputDecoration(labelText: l10n.customProviderName),
              ),
              const SizedBox(height: 16),
              OpenChatSelectField<CustomProviderProtocol>(
                label: l10n.customProviderProtocol,
                options: [
                  OpenChatSelectOption(
                    value: CustomProviderProtocol.openAi,
                    label: l10n.openAiCompatible,
                  ),
                  OpenChatSelectOption(
                    value: CustomProviderProtocol.anthropic,
                    label: l10n.anthropicCompatible,
                  ),
                ],
                value: _protocol,
                onChanged: (value) => setState(() => _protocol = value),
                palette: palette,
                height: 48,
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _urlController,
                keyboardType: TextInputType.url,
                autocorrect: false,
                enableSuggestions: false,
                decoration: InputDecoration(
                  labelText: l10n.customProviderBaseUrl,
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _keyController,
                obscureText: true,
                autocorrect: false,
                enableSuggestions: false,
                decoration: InputDecoration(
                  labelText: l10n.customProviderApiKey,
                  hintText: widget.provider == null
                      ? l10n.customProviderApiKeyOptional
                      : l10n.customProviderKeyUnchanged,
                ),
              ),
              if (_error != null) ...[
                const SizedBox(height: 12),
                Text(
                  _error!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ],
              const SizedBox(height: 24),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: Text(l10n.cancel),
                  ),
                  const SizedBox(width: 8),
                  FilledButton(onPressed: _save, child: Text(l10n.save)),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
