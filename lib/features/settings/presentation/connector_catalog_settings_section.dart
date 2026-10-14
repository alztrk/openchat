import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/domain/connector_catalog_entry.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ConnectorCatalogSettingsSection extends StatefulWidget {
  const ConnectorCatalogSettingsSection({
    required this.settingsPreferences,
    required this.serviceClient,
    required this.projectId,
    super.key,
  });

  final SettingsPreferences settingsPreferences;
  final OpenChatServiceClient? serviceClient;
  final String? projectId;

  @override
  State<ConnectorCatalogSettingsSection> createState() =>
      _ConnectorCatalogSettingsSectionState();
}

class _ConnectorCatalogSettingsSectionState
    extends State<ConnectorCatalogSettingsSection> {
  List<ConnectorCatalogEntry> _entries = const <ConnectorCatalogEntry>[];
  bool _isLoading = true;
  bool _loadFailed = false;
  bool _isSaving = false;
  String? _error;
  String? _installingId;
  late final TextEditingController _searchController;
  String _searchQuery = '';

  @override
  void initState() {
    super.initState();
    _searchController = TextEditingController();
    _load();
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _isLoading = true;
      _loadFailed = false;
      _error = null;
    });
    try {
      final entries = await widget.settingsPreferences
          .readConnectorCatalogEntries();
      if (!mounted) return;
      setState(() {
        _entries = entries;
        _isLoading = false;
        _loadFailed = false;
      });
    } on Exception {
      if (!mounted) return;
      setState(() {
        _isLoading = false;
        _loadFailed = true;
        _error = context.openchatL10n.connectorCatalogLoadFailed;
      });
    }
  }

  @override
  void didUpdateWidget(covariant ConnectorCatalogSettingsSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.settingsPreferences != widget.settingsPreferences) {
      _load();
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final query = _searchQuery.trim().toLowerCase();
    final visibleEntries = _entries
        .where((entry) {
          if (query.isEmpty) return true;
          return <String>[
            entry.name,
            entry.id,
            entry.publisher,
            entry.sourceUrl,
            entry.dataDisclosure,
          ].any((value) => value.toLowerCase().contains(query));
        })
        .toList(growable: false);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(l10n.connectorCatalogIntro),
        const SizedBox(height: 12),
        _UnverifiedNotice(message: l10n.connectorCatalogSecurityNotice),
        if (widget.projectId == null)
          Padding(
            padding: const EdgeInsets.only(top: 12),
            child: Text(l10n.connectorCatalogNoProject),
          ),
        if (_error != null) ...[
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: Text(
                  _error!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ),
              if (_loadFailed)
                TextButton(onPressed: _load, child: Text(l10n.retry)),
            ],
          ),
        ],
        const SizedBox(height: 20),
        if (_isLoading)
          const Center(child: CircularProgressIndicator())
        else if (_entries.isEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 20),
            child: Text(l10n.connectorCatalogEmpty),
          )
        else ...[
          TextField(
            controller: _searchController,
            onChanged: (value) => setState(() => _searchQuery = value),
            decoration: InputDecoration(
              labelText: l10n.connectorCatalogSearch,
              prefixIcon: const Icon(LucideIcons.search),
              suffixIcon: _searchQuery.isEmpty
                  ? null
                  : IconButton(
                      tooltip: l10n.settingsSearchClear,
                      onPressed: () {
                        _searchController.clear();
                        setState(() => _searchQuery = '');
                      },
                      icon: const Icon(LucideIcons.x),
                    ),
            ),
          ),
          const SizedBox(height: 12),
          if (visibleEntries.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 20),
              child: Text(l10n.connectorCatalogNoResults),
            ),
          for (final entry in visibleEntries) ...[
            _ConnectorEntryTile(
              entry: entry,
              installing: _installingId == entry.id,
              canInstall:
                  widget.projectId != null &&
                  widget.serviceClient != null &&
                  _installingId == null,
              onEdit: _isSaving ? null : () => _editEntry(entry),
              onDelete: _isSaving ? null : () => _deleteEntry(entry),
              onInstall: _installingId == null
                  ? () => _confirmInstall(entry)
                  : null,
            ),
            const SizedBox(height: 10),
          ],
        ],
        const SizedBox(height: 8),
        Align(
          alignment: Alignment.centerLeft,
          child: FilledButton.icon(
            onPressed: _isSaving || _isLoading || _loadFailed
                ? null
                : _addEntry,
            icon: const Icon(LucideIcons.plus),
            label: Text(l10n.connectorCatalogAdd),
          ),
        ),
      ],
    );
  }

  Future<void> _addEntry() async {
    final entry = await showDialog<ConnectorCatalogEntry>(
      context: context,
      builder: (context) => const _ConnectorEntryEditor(),
    );
    if (entry == null || !mounted) return;
    await _saveEntries([..._entries, entry]);
  }

  Future<void> _editEntry(ConnectorCatalogEntry existing) async {
    final updated = await showDialog<ConnectorCatalogEntry>(
      context: context,
      builder: (context) => _ConnectorEntryEditor(entry: existing),
    );
    if (updated == null || !mounted) return;
    final entries = _entries
        .map((entry) => entry.id == existing.id ? updated : entry)
        .toList(growable: false);
    await _saveEntries(entries);
  }

  Future<void> _deleteEntry(ConnectorCatalogEntry entry) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(context.openchatL10n.connectorCatalogDelete),
        content: Text(context.openchatL10n.connectorCatalogDeleteConfirm),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(context.openchatL10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(context.openchatL10n.connectorCatalogDelete),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    await _saveEntries(
      _entries.where((item) => item.id != entry.id).toList(growable: false),
    );
  }

  Future<void> _confirmInstall(ConnectorCatalogEntry entry) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(context.openchatL10n.connectorCatalogInstall),
        content: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(context.openchatL10n.connectorCatalogInstallPrompt),
              const SizedBox(height: 12),
              Text(
                '${context.openchatL10n.connectorCatalogPublisher}: ${entry.publisher}',
              ),
              Text(
                '${context.openchatL10n.connectorCatalogVersion}: ${entry.version}',
              ),
              const SizedBox(height: 6),
              SelectableText(
                '${context.openchatL10n.connectorCatalogSourceUrl}: ${entry.sourceUrl}',
              ),
              const SizedBox(height: 6),
              Text(
                '${context.openchatL10n.projectMcpEndpoint}: ${entry.endpoint ?? entry.program}',
              ),
              const SizedBox(height: 6),
              Text(context.openchatL10n.connectorCatalogAuthentication),
              Text(_authenticationDisclosure(context, entry)),
              if (entry.authEnvironmentVariable case final variable?)
                Text(
                  '${context.openchatL10n.connectorCatalogAuthVariable}: $variable',
                ),
              if (entry.transport == ConnectorCatalogTransport.stdio &&
                  entry.environmentVariables.isNotEmpty)
                Text(
                  '${context.openchatL10n.connectorCatalogEnvironmentVariables}: ${entry.environmentVariables.join(', ')}',
                ),
              const SizedBox(height: 6),
              Text(
                '${context.openchatL10n.connectorCatalogRequestedAccess}: ${entry.requestedAccess}',
              ),
              const SizedBox(height: 6),
              Text(entry.dataDisclosure),
              const SizedBox(height: 6),
              Text(context.openchatL10n.connectorCatalogRemovalImpact),
              const SizedBox(height: 6),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(context.openchatL10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(context.openchatL10n.connectorCatalogInstall),
          ),
        ],
      ),
    );
    if (confirmed == true && mounted) await _installEntry(entry);
  }

  Future<bool> _saveEntries(List<ConnectorCatalogEntry> entries) async {
    final ids = entries.map((entry) => entry.id).toSet();
    final names = entries.map((entry) => entry.name.toLowerCase()).toSet();
    if (ids.length != entries.length || names.length != entries.length) {
      setState(() => _error = context.openchatL10n.connectorCatalogDuplicate);
      return false;
    }
    setState(() {
      _isSaving = true;
      _error = null;
    });
    try {
      await widget.settingsPreferences.writeConnectorCatalogEntries(entries);
      if (!mounted) return false;
      setState(() {
        _entries = List<ConnectorCatalogEntry>.unmodifiable(entries);
        _isSaving = false;
      });
      return true;
    } on ArgumentError {
      if (!mounted) return false;
      setState(() {
        _isSaving = false;
        _error = context.openchatL10n.connectorCatalogInvalid;
      });
      return false;
    } on Exception {
      if (!mounted) return false;
      setState(() {
        _isSaving = false;
        _error = context.openchatL10n.connectorCatalogSaveFailed;
      });
      return false;
    }
  }

  Future<void> _installEntry(ConnectorCatalogEntry entry) async {
    final projectId = widget.projectId;
    final serviceClient = widget.serviceClient;
    if (projectId == null || serviceClient == null) return;
    setState(() {
      _installingId = entry.id;
      _error = null;
    });
    try {
      final existingCatalog = await serviceClient.call(
        'project.mcp.catalog.get',
        params: <String, Object?>{'projectId': projectId},
      );
      final existingServers = existingCatalog['servers'];
      if (existingCatalog['version'] != 1 ||
          existingServers is! List<Object?>) {
        throw const FormatException('Invalid project connector catalog.');
      }
      if (existingServers.any(
        (server) => server is Map<String, Object?> && server['id'] == entry.id,
      )) {
        throw const _DuplicateProjectConnectorException();
      }

      final rules = await widget.settingsPreferences
          .readProjectToolPermissionRules(projectId);
      final updatedRules = <String, ToolPermissionRule>{
        ...rules,
        'mcp__${entry.id}__*': ToolPermissionRule.ask,
      };
      await widget.settingsPreferences.writeProjectToolPermissionRules(
        projectId,
        updatedRules,
      );
      try {
        await serviceClient.call(
          'project.mcp.catalog.save',
          params: <String, Object?>{
            'projectId': projectId,
            'catalog': <String, Object?>{
              'version': 1,
              'servers': <Object?>[...existingServers, entry.toProjectServer()],
            },
          },
        );
      } on Object catch (saveError, saveStackTrace) {
        try {
          await widget.settingsPreferences.writeProjectToolPermissionRules(
            projectId,
            rules,
          );
        } on Object catch (rollbackError, rollbackStackTrace) {
          FlutterError.reportError(
            FlutterErrorDetails(
              exception: rollbackError,
              stack: rollbackStackTrace,
              library: 'connector_catalog',
              context: ErrorDescription(
                'while restoring connector permissions after installation failed',
              ),
            ),
          );
        }
        Error.throwWithStackTrace(saveError, saveStackTrace);
      }
      if (mounted) {
        setState(() => _installingId = null);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(context.openchatL10n.connectorCatalogInstalled),
          ),
        );
      }
    } on _DuplicateProjectConnectorException {
      if (mounted) {
        setState(() {
          _installingId = null;
          _error = context.openchatL10n.connectorCatalogDuplicate;
        });
      }
    } on Exception {
      if (mounted) {
        setState(() {
          _installingId = null;
          _error = context.openchatL10n.connectorCatalogInstallFailed;
        });
      }
    }
  }
}

String _authenticationDisclosure(
  BuildContext context,
  ConnectorCatalogEntry entry,
) {
  final l10n = context.openchatL10n;
  if (entry.transport == ConnectorCatalogTransport.streamableHttp) {
    return entry.authEnvironmentVariable == null
        ? l10n.connectorCatalogNoBearerCredential
        : l10n.connectorCatalogProjectBearerCredential;
  }
  return entry.environmentVariables.isEmpty
      ? l10n.connectorCatalogNoCredentialVariables
      : l10n.connectorCatalogProjectEnvironmentCredentials;
}

class _ConnectorEntryTile extends StatelessWidget {
  const _ConnectorEntryTile({
    required this.entry,
    required this.installing,
    required this.canInstall,
    required this.onEdit,
    required this.onDelete,
    required this.onInstall,
  });

  final ConnectorCatalogEntry entry;
  final bool installing;
  final bool canInstall;
  final VoidCallback? onEdit;
  final VoidCallback? onDelete;
  final VoidCallback? onInstall;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                const Icon(LucideIcons.plug),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    entry.name,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                IconButton(
                  tooltip: l10n.connectorCatalogEdit,
                  onPressed: onEdit,
                  icon: const Icon(LucideIcons.pencil),
                ),
                IconButton(
                  tooltip: l10n.connectorCatalogDelete,
                  onPressed: onDelete,
                  icon: const Icon(LucideIcons.trash2),
                ),
              ],
            ),
            Text('${l10n.connectorCatalogPublisher}: ${entry.publisher}'),
            Text('${l10n.connectorCatalogVersion}: ${entry.version}'),
            SelectableText(
              '${l10n.connectorCatalogSourceUrl}: ${entry.sourceUrl}',
            ),
            const SizedBox(height: 8),
            Text(
              '${l10n.connectorCatalogRequestedAccess}: ${entry.requestedAccess}',
            ),
            const SizedBox(height: 6),
            Text(entry.dataDisclosure),
            const SizedBox(height: 8),
            Align(
              alignment: Alignment.centerLeft,
              child: OutlinedButton.icon(
                onPressed: canInstall ? onInstall : null,
                icon: installing
                    ? const SizedBox.square(
                        dimension: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(LucideIcons.download),
                label: Text(l10n.connectorCatalogInstall),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _UnverifiedNotice extends StatelessWidget {
  const _UnverifiedNotice({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(12),
    decoration: BoxDecoration(
      border: Border.all(color: Theme.of(context).colorScheme.outlineVariant),
      borderRadius: BorderRadius.circular(12),
    ),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Icon(LucideIcons.shieldAlert, size: 18),
        const SizedBox(width: 10),
        Expanded(child: Text(message)),
      ],
    ),
  );
}

class _ConnectorEntryEditor extends StatefulWidget {
  const _ConnectorEntryEditor({this.entry});

  final ConnectorCatalogEntry? entry;

  @override
  State<_ConnectorEntryEditor> createState() => _ConnectorEntryEditorState();
}

class _ConnectorEntryEditorState extends State<_ConnectorEntryEditor> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _id;
  late final TextEditingController _name;
  late final TextEditingController _publisher;
  late final TextEditingController _version;
  late final TextEditingController _source;
  late final TextEditingController _requestedAccess;
  late final TextEditingController _disclosure;
  late final TextEditingController _program;
  late final TextEditingController _arguments;
  late final TextEditingController _environmentVariables;
  late final TextEditingController _endpoint;
  late final TextEditingController _authVariable;
  late ConnectorCatalogTransport _transport;

  @override
  void initState() {
    super.initState();
    final entry = widget.entry;
    _id = TextEditingController(text: entry?.id ?? '');
    _name = TextEditingController(text: entry?.name ?? '');
    _publisher = TextEditingController(text: entry?.publisher ?? '');
    _version = TextEditingController(text: entry?.version ?? '');
    _source = TextEditingController(text: entry?.sourceUrl ?? '');
    _requestedAccess = TextEditingController(
      text: entry?.requestedAccess ?? '',
    );
    _disclosure = TextEditingController(text: entry?.dataDisclosure ?? '');
    _program = TextEditingController(text: entry?.program ?? '');
    _arguments = TextEditingController(text: entry?.arguments.join('\n') ?? '');
    _environmentVariables = TextEditingController(
      text: entry?.environmentVariables.join('\n') ?? '',
    );
    _endpoint = TextEditingController(text: entry?.endpoint ?? '');
    _authVariable = TextEditingController(
      text: entry?.authEnvironmentVariable ?? '',
    );
    _transport = entry?.transport ?? ConnectorCatalogTransport.stdio;
  }

  @override
  void dispose() {
    _id.dispose();
    _name.dispose();
    _publisher.dispose();
    _version.dispose();
    _source.dispose();
    _requestedAccess.dispose();
    _disclosure.dispose();
    _program.dispose();
    _arguments.dispose();
    _environmentVariables.dispose();
    _endpoint.dispose();
    _authVariable.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return AlertDialog(
      insetPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 24),
      title: Text(
        widget.entry == null
            ? l10n.connectorCatalogAdd
            : l10n.connectorCatalogEdit,
      ),
      content: SizedBox(
        width: MediaQuery.sizeOf(context).width < 608
            ? MediaQuery.sizeOf(context).width - 48
            : 560,
        child: Form(
          key: _formKey,
          child: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                _textField(_id, l10n.connectorCatalogId, maxLength: 24),
                _textField(_name, l10n.connectorCatalogName, maxLength: 80),
                _textField(
                  _publisher,
                  l10n.connectorCatalogPublisher,
                  maxLength: 120,
                ),
                _textField(
                  _version,
                  l10n.connectorCatalogVersion,
                  maxLength: 64,
                ),
                _textField(
                  _source,
                  l10n.connectorCatalogSourceUrl,
                  maxLength: 2048,
                ),
                _textField(
                  _disclosure,
                  l10n.connectorCatalogDataDisclosure,
                  maxLength: 500,
                  maxLines: 3,
                ),
                _textField(
                  _requestedAccess,
                  l10n.connectorCatalogRequestedAccess,
                  maxLength: 500,
                  maxLines: 3,
                ),
                DropdownButtonFormField<ConnectorCatalogTransport>(
                  initialValue: _transport,
                  isExpanded: true,
                  decoration: InputDecoration(
                    labelText: l10n.connectorCatalogTransport,
                  ),
                  items: [
                    DropdownMenuItem(
                      value: ConnectorCatalogTransport.stdio,
                      child: Text(l10n.projectMcpTransportStdio),
                    ),
                    DropdownMenuItem(
                      value: ConnectorCatalogTransport.streamableHttp,
                      child: Text(l10n.projectMcpTransportHttp),
                    ),
                  ],
                  onChanged: (value) {
                    if (value != null) setState(() => _transport = value);
                  },
                ),
                if (_transport == ConnectorCatalogTransport.stdio) ...[
                  _textField(_program, l10n.connectorCatalogProgram),
                  _textField(
                    _arguments,
                    l10n.connectorCatalogArguments,
                    maxLines: 3,
                    requiredField: false,
                  ),
                  _textField(
                    _environmentVariables,
                    l10n.connectorCatalogEnvironmentVariables,
                    maxLines: 2,
                    requiredField: false,
                  ),
                ] else ...[
                  _textField(_endpoint, l10n.projectMcpEndpoint),
                  _textField(
                    _authVariable,
                    l10n.projectMcpAuthVariable,
                    requiredField: false,
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(onPressed: _save, child: Text(l10n.connectorCatalogSave)),
      ],
    );
  }

  Widget _textField(
    TextEditingController controller,
    String label, {
    int maxLength = 4096,
    int maxLines = 1,
    bool requiredField = true,
  }) => Padding(
    padding: const EdgeInsets.only(bottom: 12),
    child: TextFormField(
      controller: controller,
      maxLength: maxLength,
      maxLines: maxLines,
      decoration: InputDecoration(labelText: label),
      validator: (value) =>
          requiredField && (value == null || value.trim().isEmpty)
          ? context.openchatL10n.connectorCatalogInvalid
          : null,
    ),
  );

  void _save() {
    final l10n = context.openchatL10n;
    final form = _formKey.currentState;
    if (form == null || !form.validate()) return;
    final entry = <String, Object?>{
      'id': _id.text.trim(),
      'name': _name.text.trim(),
      'publisher': _publisher.text.trim(),
      'version': _version.text.trim(),
      'sourceUrl': _source.text.trim(),
      'requestedAccess': _requestedAccess.text.trim(),
      'dataDisclosure': _disclosure.text.trim(),
      'transport': _transport.storageValue,
      if (_transport == ConnectorCatalogTransport.stdio)
        'program': _program.text.trim(),
      'arguments': _arguments.text
          .split('\n')
          .map((line) => line.trim())
          .where((line) => line.isNotEmpty)
          .toList(growable: false),
      'environmentVariables': _environmentVariables.text
          .split('\n')
          .map((line) => line.trim())
          .where((line) => line.isNotEmpty)
          .toList(growable: false),
      if (_transport == ConnectorCatalogTransport.streamableHttp)
        'endpoint': _endpoint.text.trim(),
      if (_transport == ConnectorCatalogTransport.streamableHttp &&
          _authVariable.text.trim().isNotEmpty)
        'authEnvironmentVariable': _authVariable.text.trim(),
    };
    try {
      Navigator.of(context).pop(ConnectorCatalogEntry.fromJson(entry));
    } on FormatException {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.connectorCatalogInvalid)));
    }
  }
}

class _DuplicateProjectConnectorException implements Exception {
  const _DuplicateProjectConnectorException();
}
