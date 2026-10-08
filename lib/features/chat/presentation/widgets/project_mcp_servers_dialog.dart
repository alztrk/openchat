import 'package:flutter/material.dart';

import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ProjectMcpServersDialog extends StatefulWidget {
  const ProjectMcpServersDialog({
    required this.serviceClient,
    required this.projectId,
    required this.initialPermissionRules,
    required this.onSavePermissionRules,
    super.key,
  });

  final OpenChatServiceClient serviceClient;
  final String projectId;
  final Map<String, ToolPermissionRule> initialPermissionRules;
  final Future<void> Function(Map<String, ToolPermissionRule> rules)
  onSavePermissionRules;

  @override
  State<ProjectMcpServersDialog> createState() =>
      _ProjectMcpServersDialogState();
}

class _ProjectMcpServersDialogState extends State<ProjectMcpServersDialog> {
  List<_McpServer> _servers = const <_McpServer>[];
  bool _isLoading = true;
  bool _loadFailed = false;
  bool _isSaving = false;
  String? _error;
  final Set<String> _checkingServerIds = <String>{};
  final Map<String, String> _serverCheckResults = <String, String>{};

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _isLoading = true;
      _loadFailed = false;
      _error = null;
    });
    try {
      final catalog = await widget.serviceClient.call(
        'project.mcp.catalog.get',
        params: <String, Object?>{'projectId': widget.projectId},
      );
      final rawServers = catalog['servers'];
      if (catalog['version'] != 1 || rawServers is! List<Object?>) {
        throw const FormatException('Invalid project MCP catalog.');
      }
      final servers = rawServers.map((server) {
        final id = _McpServer.readId(server);
        return _McpServer.fromObject(
          server,
          permissionRule:
              widget.initialPermissionRules[_permissionKey(id)] ??
              ToolPermissionRule.inherit,
        );
      }).toList();
      if (mounted) {
        setState(() {
          _servers = servers;
          _isLoading = false;
          _loadFailed = false;
        });
      }
    } on Exception {
      if (mounted) {
        setState(() {
          _isLoading = false;
          _loadFailed = true;
          _error = context.openchatL10n.projectMcpLoadFailed;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return AlertDialog(
      title: Text(l10n.projectMcpTitle),
      content: SizedBox(
        width: 560,
        height: MediaQuery.sizeOf(context).height * 0.65,
        child: _isLoading
            ? const Center(child: CircularProgressIndicator())
            : ListView(
                children: [
                  if (_servers.isEmpty)
                    Padding(
                      padding: const EdgeInsets.symmetric(vertical: 12),
                      child: Text(
                        l10n.projectMcpEmpty,
                        textAlign: TextAlign.center,
                      ),
                    ),
                  Text(l10n.projectMcpDescription),
                  const SizedBox(height: 8),
                  Text(
                    l10n.projectMcpPermissionsLocal,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  const SizedBox(height: 8),
                  Text(
                    l10n.projectMcpNoCredentials,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  const SizedBox(height: 12),
                  for (final entry in _servers.asMap().entries)
                    _serverTile(entry.key, entry.value, l10n),
                  if (_error case final error?) ...[
                    const SizedBox(height: 8),
                    Text(
                      error,
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ],
                ],
              ),
      ),
      actions: [
        TextButton(
          onPressed: _isSaving ? null : () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        if (_loadFailed) TextButton(onPressed: _load, child: Text(l10n.retry)),
        if (!_isLoading && !_loadFailed)
          TextButton.icon(
            onPressed: _isSaving ? null : _addServer,
            icon: const Icon(Icons.add),
            label: Text(l10n.projectMcpAdd),
          ),
        if (!_isLoading && !_loadFailed)
          FilledButton(
            onPressed: _isSaving ? null : _save,
            child: _isSaving
                ? const SizedBox.square(
                    dimension: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : Text(l10n.save),
          ),
      ],
    );
  }

  Widget _serverTile(int index, _McpServer server, AppLocalizations l10n) {
    return ListTile(
      key: ValueKey<String>('project-mcp-server-${server.id}'),
      contentPadding: EdgeInsets.zero,
      title: Text(server.id),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(server.program),
          Text(
            '${l10n.projectMcpPermissionScope}: ${_permissionLabel(l10n, server.permissionRule)}',
          ),
          if (_serverCheckResults[server.id] case final result?) Text(result),
        ],
      ),
      leading: Switch(
        value: server.enabled,
        onChanged: _isSaving
            ? null
            : (enabled) =>
                  _replaceServer(index, server.copyWith(enabled: enabled)),
      ),
      trailing: Wrap(
        children: [
          IconButton(
            tooltip: l10n.projectMcpCheck,
            onPressed:
                _isSaving ||
                    !server.enabled ||
                    _checkingServerIds.contains(server.id)
                ? null
                : () => _checkServer(server),
            icon: _checkingServerIds.contains(server.id)
                ? const SizedBox.square(
                    dimension: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.wifi_tethering_outlined),
          ),
          IconButton(
            tooltip: l10n.projectMcpEdit,
            onPressed: _isSaving ? null : () => _editServer(index),
            icon: const Icon(Icons.edit_outlined),
          ),
          IconButton(
            tooltip: l10n.projectMcpRemove,
            onPressed: _isSaving ? null : () => _removeServer(index),
            icon: const Icon(Icons.delete_outline),
          ),
        ],
      ),
    );
  }

  String _permissionLabel(
    AppLocalizations l10n,
    ToolPermissionRule permission,
  ) => switch (permission) {
    ToolPermissionRule.inherit => l10n.projectToolRuleInherit,
    ToolPermissionRule.ask => l10n.projectToolRuleAsk,
    ToolPermissionRule.allow => l10n.projectToolRuleAllow,
    ToolPermissionRule.deny => l10n.projectToolRuleDeny,
  };

  Future<void> _addServer() async {
    final server = await showDialog<_McpServer>(
      context: context,
      builder: (context) => const _McpServerEditor(),
    );
    if (server == null || !mounted) return;
    if (_servers.any((existing) => existing.id == server.id)) {
      setState(() => _error = context.openchatL10n.projectMcpDuplicateId);
      return;
    }
    setState(() {
      _servers = [..._servers, server];
      _error = null;
    });
  }

  Future<void> _editServer(int index) async {
    final updated = await showDialog<_McpServer>(
      context: context,
      builder: (context) => _McpServerEditor(server: _servers[index]),
    );
    if (updated == null || !mounted) return;
    if (_servers.asMap().entries.any(
      (entry) => entry.key != index && entry.value.id == updated.id,
    )) {
      setState(() => _error = context.openchatL10n.projectMcpDuplicateId);
      return;
    }
    _replaceServer(index, updated);
  }

  void _replaceServer(int index, _McpServer server) {
    setState(() {
      final updated = List<_McpServer>.of(_servers);
      updated[index] = server;
      _servers = updated;
      _error = null;
      _serverCheckResults.remove(server.id);
    });
  }

  void _removeServer(int index) {
    final id = _servers[index].id;
    setState(() {
      _servers = List<_McpServer>.of(_servers)..removeAt(index);
      _serverCheckResults.remove(id);
      _error = null;
    });
  }

  Future<void> _checkServer(_McpServer server) async {
    setState(() {
      _checkingServerIds.add(server.id);
      _serverCheckResults.remove(server.id);
    });
    try {
      final result = await widget.serviceClient.call(
        'project.mcp.server.check',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'server': server.toJson(),
        },
        timeout: const Duration(seconds: 40),
      );
      final toolCount = result['toolCount'];
      if (result['status'] != 'connected' ||
          toolCount is! int ||
          toolCount < 0) {
        throw const FormatException('Invalid MCP check result.');
      }
      if (mounted) {
        setState(() {
          _serverCheckResults[server.id] = context.openchatL10n
              .projectMcpConnected(toolCount);
        });
      }
    } on Exception {
      if (mounted) {
        setState(() {
          _serverCheckResults[server.id] =
              context.openchatL10n.projectMcpCheckFailed;
        });
      }
    } finally {
      if (mounted) {
        setState(() => _checkingServerIds.remove(server.id));
      }
    }
  }

  Future<void> _save() async {
    setState(() {
      _isSaving = true;
      _error = null;
    });
    try {
      await widget.serviceClient.call(
        'project.mcp.catalog.save',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'catalog': <String, Object?>{
            'version': 1,
            'servers': _servers.map((server) => server.toJson()).toList(),
          },
        },
      );
      final updatedRules = <String, ToolPermissionRule>{
        for (final entry in widget.initialPermissionRules.entries)
          if (!entry.key.startsWith('mcp__')) entry.key: entry.value,
        for (final server in _servers)
          if (server.permissionRule != ToolPermissionRule.inherit)
            _permissionKey(server.id): server.permissionRule,
      };
      await widget.onSavePermissionRules(updatedRules);
      if (mounted) Navigator.of(context).pop();
    } on Exception {
      if (mounted) {
        setState(() {
          _isSaving = false;
          _error = context.openchatL10n.projectMcpSaveFailed;
        });
      }
    }
  }
}

class _McpServer {
  const _McpServer({
    required this.id,
    required this.enabled,
    required this.program,
    required this.arguments,
    required this.permissionRule,
  });

  final String id;
  final bool enabled;
  final String program;
  final List<String> arguments;
  final ToolPermissionRule permissionRule;

  static String readId(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('Invalid project MCP server.');
    }
    final id = value['id'];
    if (id is! String) throw const FormatException('Invalid project MCP ID.');
    return id;
  }

  factory _McpServer.fromObject(
    Object? value, {
    required ToolPermissionRule permissionRule,
  }) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('Invalid project MCP server.');
    }
    final id = value['id'];
    final enabled = value['enabled'];
    final program = value['program'];
    final arguments = value['arguments'];
    if (id is! String || enabled is! bool || program is! String) {
      throw const FormatException('Invalid project MCP arguments.');
    }
    final List<String> parsedArguments;
    if (arguments == null) {
      parsedArguments = const <String>[];
    } else if (arguments is List<Object?> &&
        arguments.every((item) => item is String)) {
      parsedArguments = arguments.whereType<String>().toList();
    } else {
      throw const FormatException('Invalid project MCP arguments.');
    }
    return _McpServer(
      id: id,
      enabled: enabled,
      program: program,
      arguments: parsedArguments,
      permissionRule: permissionRule,
    );
  }

  _McpServer copyWith({bool? enabled}) => _McpServer(
    id: id,
    enabled: enabled ?? this.enabled,
    program: program,
    arguments: arguments,
    permissionRule: permissionRule,
  );

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'enabled': enabled,
    'program': program,
    'arguments': arguments,
  };
}

String _permissionKey(String serverId) => 'mcp__${serverId}__*';

class _McpServerEditor extends StatefulWidget {
  const _McpServerEditor({this.server});

  final _McpServer? server;

  @override
  State<_McpServerEditor> createState() => _McpServerEditorState();
}

class _McpServerEditorState extends State<_McpServerEditor> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _id = TextEditingController(
    text: widget.server?.id ?? '',
  );
  late final TextEditingController _program = TextEditingController(
    text: widget.server?.program ?? '',
  );
  late final TextEditingController _arguments = TextEditingController(
    text: widget.server?.arguments.join('\n') ?? '',
  );
  bool _enabled = true;
  ToolPermissionRule _permissionRule = ToolPermissionRule.inherit;

  @override
  void initState() {
    super.initState();
    _enabled = widget.server?.enabled ?? true;
    _permissionRule =
        widget.server?.permissionRule ?? ToolPermissionRule.inherit;
  }

  @override
  void dispose() {
    _id.dispose();
    _program.dispose();
    _arguments.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return AlertDialog(
      title: Text(
        widget.server == null ? l10n.projectMcpAdd : l10n.projectMcpEdit,
      ),
      content: SizedBox(
        width: 480,
        child: Form(
          key: _formKey,
          child: ListView(
            shrinkWrap: true,
            children: [
              TextFormField(
                controller: _id,
                enabled: widget.server == null,
                maxLength: 24,
                decoration: InputDecoration(labelText: l10n.projectMcpServerId),
                validator: (value) {
                  final text = value ?? '';
                  return RegExp(r'^[a-z0-9_]{1,24}$').hasMatch(text)
                      ? null
                      : l10n.projectMcpInvalidId;
                },
              ),
              TextFormField(
                controller: _program,
                decoration: InputDecoration(labelText: l10n.projectMcpProgram),
                validator: (value) => value == null || value.trim().isEmpty
                    ? l10n.projectMcpProgramRequired
                    : null,
              ),
              TextFormField(
                controller: _arguments,
                minLines: 2,
                maxLines: 5,
                decoration: InputDecoration(
                  labelText: l10n.projectMcpArguments,
                  helperText: l10n.projectMcpArgumentsHint,
                ),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: Text(l10n.projectMcpEnabled),
                value: _enabled,
                onChanged: (value) => setState(() => _enabled = value),
              ),
              DropdownButtonFormField<ToolPermissionRule>(
                key: const ValueKey<String>('project-mcp-permission-rule'),
                initialValue: _permissionRule,
                isExpanded: true,
                decoration: InputDecoration(
                  labelText: l10n.projectMcpPermissionScope,
                ),
                items: [
                  DropdownMenuItem(
                    value: ToolPermissionRule.inherit,
                    child: Text(l10n.projectToolRuleInherit),
                  ),
                  DropdownMenuItem(
                    value: ToolPermissionRule.ask,
                    child: Text(l10n.projectToolRuleAsk),
                  ),
                  DropdownMenuItem(
                    value: ToolPermissionRule.allow,
                    child: Text(l10n.projectToolRuleAllow),
                  ),
                  DropdownMenuItem(
                    value: ToolPermissionRule.deny,
                    child: Text(l10n.projectToolRuleDeny),
                  ),
                ],
                onChanged: (value) {
                  if (value != null) setState(() => _permissionRule = value);
                },
              ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(onPressed: _save, child: Text(l10n.save)),
      ],
    );
  }

  void _save() {
    final form = _formKey.currentState;
    if (form == null || !form.validate()) return;
    final arguments = _arguments.text
        .split('\n')
        .map(
          (argument) => argument.endsWith('\r')
              ? argument.substring(0, argument.length - 1)
              : argument,
        )
        .where((argument) => argument.isNotEmpty)
        .toList();
    Navigator.of(context).pop(
      _McpServer(
        id: _id.text,
        enabled: _enabled,
        program: _program.text,
        arguments: arguments,
        permissionRule: _permissionRule,
      ),
    );
  }
}
