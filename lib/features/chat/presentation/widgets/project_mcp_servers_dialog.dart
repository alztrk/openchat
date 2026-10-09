import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/project_tool_permission_select.dart';
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
  List<_McpServer> _savedServers = const <_McpServer>[];
  bool _isLoading = true;
  bool _loadFailed = false;
  bool _isSaving = false;
  String? _error;
  final Set<String> _checkingServerIds = <String>{};
  final Map<String, String> _serverCheckResults = <String, String>{};
  final Map<String, List<String>> _serverToolNames = <String, List<String>>{};
  late final Map<String, ToolPermissionRule> _toolPermissionRules = {
    for (final entry in widget.initialPermissionRules.entries)
      if (entry.key.startsWith('mcp__') && !entry.key.endsWith('__*'))
        entry.key: entry.value,
  };

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
          _savedServers = List<_McpServer>.unmodifiable(servers);
          for (final server in servers) {
            final prefix = 'mcp__${server.id}__';
            _serverToolNames[server.id] =
                _toolPermissionRules.keys
                    .where(
                      (key) =>
                          key.startsWith(prefix) &&
                          !key.endsWith('__*') &&
                          _isValidMcpToolName(key.substring(prefix.length)),
                    )
                    .map((key) => key.substring(prefix.length))
                    .toSet()
                    .toList()
                  ..sort();
          }
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
            icon: const Icon(LucideIcons.plus),
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
          Text(
            server.transport == 'stdio'
                ? server.program ?? ''
                : server.endpoint ?? '',
          ),
          Text(
            '${l10n.projectMcpPermissionScope}: ${_permissionLabel(l10n, server.permissionRule)}',
          ),
          for (final toolName
              in _serverToolNames[server.id] ?? const <String>[])
            ProjectToolPermissionSelect(
              key: ValueKey<String>(
                'project-mcp-tool-rule-${server.id}-$toolName',
              ),
              label: '$toolName · ${l10n.projectMcpPermissionScope}',
              value:
                  _toolPermissionRules[_toolPermissionKey(
                    server.id,
                    toolName,
                  )] ??
                  ToolPermissionRule.inherit,
              onChanged:
                  _isSaving || server.permissionRule == ToolPermissionRule.deny
                  ? null
                  : (value) {
                      final key = _toolPermissionKey(server.id, toolName);
                      setState(() {
                        if (value == ToolPermissionRule.inherit) {
                          _toolPermissionRules.remove(key);
                        } else {
                          _toolPermissionRules[key] = value;
                        }
                      });
                    },
            ),
          if (server.environmentVariables.isNotEmpty)
            Text(
              '${l10n.projectMcpEnvironmentVariables}: ${server.environmentVariables.join(', ')}',
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
            key: ValueKey<String>('project-mcp-credentials-${server.id}'),
            onPressed: server.credentialNames.isEmpty
                ? null
                : () => _manageCredentials(server),
            tooltip: l10n.projectMcpCredentials,
            icon: const Icon(LucideIcons.keyRound),
          ),
          IconButton(
            key: ValueKey<String>('project-mcp-check-server-${server.id}'),
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
                : const Icon(LucideIcons.radio),
          ),
          IconButton(
            tooltip: l10n.projectMcpEdit,
            onPressed: _isSaving ? null : () => _editServer(index),
            icon: const Icon(LucideIcons.pencil),
          ),
          IconButton(
            tooltip: l10n.projectMcpRemove,
            onPressed: _isSaving ? null : () => _removeServer(index),
            icon: const Icon(LucideIcons.trash2),
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
      _serverToolNames.remove(server.id);
      _serverCheckResults.remove(server.id);
    });
  }

  void _removeServer(int index) {
    final id = _servers[index].id;
    setState(() {
      _servers = List<_McpServer>.of(_servers)..removeAt(index);
      _serverToolNames.remove(id);
      _serverCheckResults.remove(id);
      _error = null;
    });
  }

  Future<void> _checkServer(_McpServer server) async {
    setState(() {
      _checkingServerIds.add(server.id);
      _serverToolNames.remove(server.id);
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
      final rawToolNames = result['toolNames'];
      if (result['status'] != 'connected' ||
          toolCount is! int ||
          toolCount < 0 ||
          rawToolNames is! List<Object?> ||
          rawToolNames.length != toolCount ||
          rawToolNames.any(
            (name) => name is! String || !_isValidMcpToolName(name),
          )) {
        throw const FormatException('Invalid MCP check result.');
      }
      if (mounted) {
        setState(() {
          _serverToolNames[server.id] =
              rawToolNames.cast<String>().toSet().toList()..sort();
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

  Future<void> _manageCredentials(_McpServer server) async {
    await showDialog<void>(
      context: context,
      builder: (context) => _McpCredentialsDialog(
        serviceClient: widget.serviceClient,
        projectId: widget.projectId,
        server: server,
      ),
    );
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
      await _removeStaleCredentials(_savedServers, _servers);
      _savedServers = List<_McpServer>.unmodifiable(_servers);
      final updatedRules = <String, ToolPermissionRule>{
        for (final entry in widget.initialPermissionRules.entries)
          if (!entry.key.startsWith('mcp__')) entry.key: entry.value,
        for (final server in _servers)
          if (server.permissionRule != ToolPermissionRule.inherit)
            _permissionKey(server.id): server.permissionRule,
        for (final entry in _toolPermissionRules.entries)
          if (_servers.any(
            (server) => entry.key.startsWith('mcp__${server.id}__'),
          ))
            entry.key: entry.value,
      };
      await widget.onSavePermissionRules(updatedRules);
      if (mounted) Navigator.of(context).pop();
    } on _McpCredentialCleanupException {
      if (mounted) {
        setState(() {
          _isSaving = false;
          _error = context.openchatL10n.projectMcpCredentialUnavailable;
        });
      }
    } on Exception {
      if (mounted) {
        setState(() {
          _isSaving = false;
          _error = context.openchatL10n.projectMcpSaveFailed;
        });
      }
    }
  }

  Future<void> _removeStaleCredentials(
    List<_McpServer> previousServers,
    List<_McpServer> currentServers,
  ) async {
    for (final previous in previousServers) {
      final currentIndex = currentServers.indexWhere(
        (server) => server.id == previous.id,
      );
      final current = currentIndex < 0 ? null : currentServers[currentIndex];
      for (final name in previous.credentialNames) {
        if (current?.credentialNames.contains(name) ?? false) continue;
        try {
          await widget.serviceClient.call(
            'project.mcp.credential.remove',
            params: <String, Object?>{
              'projectId': widget.projectId,
              'serverId': previous.id,
              'environmentName': name,
            },
          );
        } on Exception {
          throw const _McpCredentialCleanupException();
        }
      }
    }
  }
}

class _McpCredentialCleanupException implements Exception {
  const _McpCredentialCleanupException();
}

class _McpServer {
  const _McpServer({
    required this.id,
    required this.enabled,
    required this.transport,
    required this.program,
    required this.arguments,
    required this.endpoint,
    required this.authEnvironmentVariable,
    required this.permissionRule,
    required this.environmentVariables,
  });

  final String id;
  final bool enabled;
  final String transport;
  final String? program;
  final List<String> arguments;
  final String? endpoint;
  final String? authEnvironmentVariable;
  final ToolPermissionRule permissionRule;
  final List<String> environmentVariables;

  List<String> get credentialNames {
    return <String>{...environmentVariables, ?authEnvironmentVariable}.toList();
  }

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
    final transport = value['transport'] ?? 'stdio';
    final endpoint = value['endpoint'];
    final authEnvironmentVariable = value['authEnvironmentVariable'];
    final arguments = value['arguments'];
    final environmentVariables = value['environmentVariables'];
    final parsedProgram = program is String ? program : null;
    final parsedEndpoint = endpoint is String ? endpoint : null;
    final parsedAuthEnvironmentVariable = authEnvironmentVariable is String
        ? authEnvironmentVariable
        : null;
    if (id is! String ||
        enabled is! bool ||
        transport is! String ||
        (program != null && program is! String) ||
        (endpoint != null && endpoint is! String) ||
        (authEnvironmentVariable != null &&
            authEnvironmentVariable is! String)) {
      throw const FormatException('Invalid project MCP arguments.');
    }
    if (transport != 'stdio' && transport != 'streamableHttp') {
      throw const FormatException('Invalid project MCP transport.');
    }
    if ((transport == 'stdio' && program is! String) ||
        (transport == 'streamableHttp' && endpoint is! String)) {
      throw const FormatException('Invalid project MCP endpoint.');
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
    final List<String> parsedEnvironmentVariables;
    if (environmentVariables == null) {
      parsedEnvironmentVariables = const <String>[];
    } else if (environmentVariables is List<Object?> &&
        environmentVariables.every((item) => item is String)) {
      parsedEnvironmentVariables = environmentVariables
          .whereType<String>()
          .toList();
    } else {
      throw const FormatException('Invalid project MCP environment variables.');
    }
    return _McpServer(
      id: id,
      enabled: enabled,
      transport: transport,
      program: parsedProgram,
      arguments: parsedArguments,
      endpoint: parsedEndpoint,
      authEnvironmentVariable: parsedAuthEnvironmentVariable,
      permissionRule: permissionRule,
      environmentVariables: parsedEnvironmentVariables,
    );
  }

  _McpServer copyWith({bool? enabled}) => _McpServer(
    id: id,
    enabled: enabled ?? this.enabled,
    transport: transport,
    program: program,
    arguments: arguments,
    endpoint: endpoint,
    authEnvironmentVariable: authEnvironmentVariable,
    permissionRule: permissionRule,
    environmentVariables: environmentVariables,
  );

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'enabled': enabled,
    'transport': transport,
    if (program != null) 'program': program,
    'arguments': arguments,
    'environmentVariables': environmentVariables,
    if (endpoint != null) 'endpoint': endpoint,
    if (authEnvironmentVariable != null)
      'authEnvironmentVariable': authEnvironmentVariable,
  };
}

String _permissionKey(String serverId) => 'mcp__${serverId}__*';
String _toolPermissionKey(String serverId, String toolName) =>
    'mcp__${serverId}__$toolName';

bool _isValidMcpToolName(String value) =>
    value.isNotEmpty && RegExp(r'^[A-Za-z0-9_-]+$').hasMatch(value);

bool _isValidMcpEnvironmentName(String value) {
  if (!RegExp(r'^[A-Za-z_][A-Za-z0-9_]{0,127}$').hasMatch(value)) {
    return false;
  }
  return !<String>{
    'PATH',
    'SYSTEMROOT',
    'WINDIR',
    'PATHEXT',
    'COMSPEC',
    'USERPROFILE',
    'APPDATA',
    'LOCALAPPDATA',
    'TEMP',
    'TMP',
  }.contains(value.toUpperCase());
}

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
  late final TextEditingController _endpoint = TextEditingController(
    text: widget.server?.endpoint ?? '',
  );
  late final TextEditingController _authEnvironmentVariable =
      TextEditingController(text: widget.server?.authEnvironmentVariable ?? '');
  late final TextEditingController _arguments = TextEditingController(
    text: widget.server?.arguments.join('\n') ?? '',
  );
  late final TextEditingController _environmentVariables =
      TextEditingController(
        text: widget.server?.environmentVariables.join('\n') ?? '',
      );
  bool _enabled = true;
  String _transport = 'stdio';
  ToolPermissionRule _permissionRule = ToolPermissionRule.inherit;

  @override
  void initState() {
    super.initState();
    _enabled = widget.server?.enabled ?? true;
    _transport = widget.server?.transport ?? 'stdio';
    _permissionRule =
        widget.server?.permissionRule ?? ToolPermissionRule.inherit;
  }

  @override
  void dispose() {
    _id.dispose();
    _program.dispose();
    _endpoint.dispose();
    _authEnvironmentVariable.dispose();
    _arguments.dispose();
    _environmentVariables.dispose();
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
              OpenChatSelectField<String>(
                key: const ValueKey<String>('project-mcp-transport'),
                label: l10n.projectMcpTransport,
                value: _transport,
                palette: OpenChatPalette.of(context),
                options: [
                  OpenChatSelectOption<String>(
                    value: 'stdio',
                    label: l10n.projectMcpTransportStdio,
                  ),
                  OpenChatSelectOption<String>(
                    value: 'streamableHttp',
                    label: l10n.projectMcpTransportHttp,
                  ),
                ],
                onChanged: (value) => setState(() => _transport = value),
              ),
              if (_transport == 'stdio') ...[
                TextFormField(
                  controller: _program,
                  decoration: InputDecoration(
                    labelText: l10n.projectMcpProgram,
                  ),
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
                TextFormField(
                  controller: _environmentVariables,
                  minLines: 2,
                  maxLines: 4,
                  decoration: InputDecoration(
                    labelText: l10n.projectMcpEnvironmentVariables,
                    helperText: l10n.projectMcpEnvironmentVariablesHint,
                  ),
                  validator: (value) {
                    final names = (value ?? '')
                        .split('\n')
                        .map((name) => name.trim())
                        .where((name) => name.isNotEmpty)
                        .toList();
                    final unique = names.toSet();
                    return names.length <= 32 &&
                            names.every(_isValidMcpEnvironmentName) &&
                            unique.length == names.length
                        ? null
                        : l10n.projectMcpEnvironmentVariablesInvalid;
                  },
                ),
              ] else ...[
                TextFormField(
                  controller: _endpoint,
                  decoration: InputDecoration(
                    labelText: l10n.projectMcpEndpoint,
                    helperText: l10n.projectMcpEndpointHint,
                  ),
                  validator: (value) {
                    final uri = Uri.tryParse((value ?? '').trim());
                    return uri != null &&
                            uri.scheme == 'https' &&
                            uri.host.isNotEmpty &&
                            uri.userInfo.isEmpty &&
                            uri.query.isEmpty &&
                            uri.fragment.isEmpty
                        ? null
                        : l10n.projectMcpEndpointRequired;
                  },
                ),
                TextFormField(
                  controller: _authEnvironmentVariable,
                  decoration: InputDecoration(
                    labelText: l10n.projectMcpAuthVariable,
                    helperText: l10n.projectMcpAuthVariableHint,
                  ),
                  validator: (value) {
                    final name = (value ?? '').trim();
                    return name.isEmpty || _isValidMcpEnvironmentName(name)
                        ? null
                        : l10n.projectMcpEnvironmentVariablesInvalid;
                  },
                ),
              ],
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: Text(l10n.projectMcpEnabled),
                value: _enabled,
                onChanged: (value) => setState(() => _enabled = value),
              ),
              ProjectToolPermissionSelect(
                key: const ValueKey<String>('project-mcp-permission-rule'),
                label: l10n.projectMcpPermissionScope,
                value: _permissionRule,
                onChanged: (value) => setState(() => _permissionRule = value),
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
        transport: _transport,
        program: _transport == 'stdio' ? _program.text : null,
        arguments: _transport == 'stdio' ? arguments : const <String>[],
        endpoint: _transport == 'streamableHttp' ? _endpoint.text.trim() : null,
        authEnvironmentVariable:
            _transport == 'streamableHttp' &&
                _authEnvironmentVariable.text.trim().isNotEmpty
            ? _authEnvironmentVariable.text.trim()
            : null,
        permissionRule: _permissionRule,
        environmentVariables: _transport == 'stdio'
            ? _environmentVariables.text
                  .split('\n')
                  .map((name) => name.trim())
                  .where((name) => name.isNotEmpty)
                  .toList()
            : const <String>[],
      ),
    );
  }
}

class _McpCredentialsDialog extends StatefulWidget {
  const _McpCredentialsDialog({
    required this.serviceClient,
    required this.projectId,
    required this.server,
  });

  final OpenChatServiceClient serviceClient;
  final String projectId;
  final _McpServer server;

  @override
  State<_McpCredentialsDialog> createState() => _McpCredentialsDialogState();
}

class _McpCredentialsDialogState extends State<_McpCredentialsDialog> {
  final Map<String, TextEditingController> _secrets =
      <String, TextEditingController>{};
  final Map<String, bool?> _stored = <String, bool?>{};
  final Map<String, String> _errors = <String, String>{};
  final Set<String> _saving = <String>{};

  @override
  void initState() {
    super.initState();
    for (final name in widget.server.credentialNames) {
      _secrets[name] = TextEditingController();
      _stored[name] = null;
      _checkStatus(name);
    }
  }

  @override
  void dispose() {
    for (final controller in _secrets.values) {
      controller.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return AlertDialog(
      title: Text(l10n.projectMcpCredentials),
      content: SizedBox(
        width: 480,
        child: widget.server.credentialNames.isEmpty
            ? Text(l10n.projectMcpEnvironmentVariablesRequired)
            : ListView(
                shrinkWrap: true,
                children: [
                  Text(l10n.projectMcpCredentialHint),
                  for (final name in widget.server.credentialNames)
                    Padding(
                      padding: const EdgeInsets.only(top: 16),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(name),
                          TextField(
                            key: ValueKey<String>('project-mcp-secret-$name'),
                            controller: _secrets[name],
                            obscureText: true,
                            autocorrect: false,
                            enableSuggestions: false,
                            decoration: InputDecoration(
                              labelText: l10n.projectMcpSecret,
                              helperText: _stored[name] == true
                                  ? l10n.projectMcpCredentialStored
                                  : _stored[name] == false
                                  ? l10n.projectMcpCredentialMissing
                                  : null,
                            ),
                          ),
                          if (_errors[name] case final error?)
                            Text(
                              error,
                              style: TextStyle(
                                color: Theme.of(context).colorScheme.error,
                              ),
                            ),
                          Wrap(
                            spacing: 8,
                            children: [
                              TextButton(
                                onPressed: _saving.contains(name)
                                    ? null
                                    : () => _save(name),
                                child: Text(l10n.projectMcpCredentialSave),
                              ),
                              TextButton(
                                onPressed: _saving.contains(name)
                                    ? null
                                    : () => _remove(name),
                                child: Text(l10n.projectMcpCredentialRemove),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                ],
              ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.close),
        ),
      ],
    );
  }

  Future<void> _checkStatus(String name) async {
    try {
      final result = await widget.serviceClient.call(
        'project.mcp.credential.status',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'serverId': widget.server.id,
          'environmentName': name,
        },
      );
      final stored = result['stored'];
      if (stored is! bool) {
        throw const FormatException('Invalid MCP credential status.');
      }
      if (mounted) setState(() => _stored[name] = stored);
    } on Exception {
      if (mounted) {
        setState(
          () => _errors[name] =
              context.openchatL10n.projectMcpCredentialUnavailable,
        );
      }
    }
  }

  Future<void> _save(String name) async {
    final secret = _secrets[name]?.text ?? '';
    if (secret.isEmpty) {
      setState(
        () => _errors[name] = context.openchatL10n.projectMcpSecretRequired,
      );
      return;
    }
    setState(() {
      _saving.add(name);
      _errors.remove(name);
    });
    try {
      await widget.serviceClient.call(
        'project.mcp.credential.set',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'serverId': widget.server.id,
          'environmentName': name,
          'secret': secret,
        },
      );
      _secrets[name]?.clear();
      if (mounted) setState(() => _stored[name] = true);
    } on OpenChatServiceException catch (error) {
      if (mounted) {
        setState(() {
          _errors[name] = error.code == 'mcp_credential_too_large'
              ? context.openchatL10n.projectMcpSecretTooLarge
              : context.openchatL10n.projectMcpCredentialUnavailable;
        });
      }
    } on Exception {
      if (mounted) {
        setState(
          () => _errors[name] =
              context.openchatL10n.projectMcpCredentialUnavailable,
        );
      }
    } finally {
      if (mounted) setState(() => _saving.remove(name));
    }
  }

  Future<void> _remove(String name) async {
    setState(() {
      _saving.add(name);
      _errors.remove(name);
    });
    try {
      await widget.serviceClient.call(
        'project.mcp.credential.remove',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'serverId': widget.server.id,
          'environmentName': name,
        },
      );
      if (mounted) setState(() => _stored[name] = false);
    } on Exception {
      if (mounted) {
        setState(
          () => _errors[name] =
              context.openchatL10n.projectMcpCredentialUnavailable,
        );
      }
    } finally {
      if (mounted) setState(() => _saving.remove(name));
    }
  }
}
