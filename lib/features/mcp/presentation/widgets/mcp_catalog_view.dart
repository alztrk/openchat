import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_surface_card.dart';
import 'package:openchat/features/mcp/data/global_mcp_repository.dart';
import 'package:openchat/features/mcp/domain/global_mcp_server.dart';
import 'package:openchat/features/mcp/presentation/widgets/mcp_env_dialog.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class McpCatalogView extends StatefulWidget {
  const McpCatalogView({
    required this.repository,
    super.key,
  });

  final GlobalMcpRepository repository;

  @override
  State<McpCatalogView> createState() => _McpCatalogViewState();
}

class _McpCatalogViewState extends State<McpCatalogView> {
  final TextEditingController _searchController = TextEditingController();
  final Set<String> _probingServerIds = {};
  List<GlobalMcpServer> _servers = const [];
  bool _isLoading = true;
  String _selectedCategory = 'All';
  String _searchQuery = '';

  @override
  void initState() {
    super.initState();
    _loadServers();
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _loadServers() async {
    setState(() => _isLoading = true);
    final list = await widget.repository.loadAllServers();
    if (mounted) {
      setState(() {
        _servers = list;
        _isLoading = false;
      });
    }
  }

  Future<void> _installServer(GlobalMcpServer server) async {
    final updated = await widget.repository.installServer(server);
    if (!mounted) return;
    await _loadServers();
    if (!mounted) return;
    showOpenChatToast(
      context,
      context.openchatL10n.mcpEnabledSuccess,
      type: OpenChatToastType.success,
    );

    // If server requires credentials, prompt user right away
    if (updated.requiredEnvKeys.isNotEmpty && mounted) {
      await McpEnvDialog.show(
        context: context,
        server: updated,
        repository: widget.repository,
      );
    }
  }

  Future<void> _toggleEnabled(GlobalMcpServer server, bool enabled) async {
    final l10n = context.openchatL10n;
    await widget.repository.toggleEnabled(server.id, enabled);
    if (!mounted) return;
    await _loadServers();
    if (!mounted) return;
    showOpenChatToast(
      context,
      enabled ? l10n.mcpEnabledSuccess : l10n.mcpDisabledSuccess,
      type: enabled ? OpenChatToastType.success : OpenChatToastType.info,
    );
  }

  Future<void> _probeServer(GlobalMcpServer server) async {
    final l10n = context.openchatL10n;
    setState(() => _probingServerIds.add(server.id));
    try {
      await widget.repository.probeTools(server);
      if (!mounted) return;
      await _loadServers();
      if (!mounted) return;
      showOpenChatToast(
        context,
        l10n.mcpDiscoverSuccess,
        type: OpenChatToastType.success,
      );
    } catch (_) {
      if (!mounted) return;
      showOpenChatToast(
        context,
        l10n.mcpDiscoverFailed,
        type: OpenChatToastType.warning,
      );
    } finally {
      if (mounted) {
        setState(() => _probingServerIds.remove(server.id));
      }
    }
  }

  Future<void> _uninstallServer(GlobalMcpServer server) async {
    final l10n = context.openchatL10n;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(server.name),
        content: Text(l10n.mcpUninstallConfirm),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: Text(l10n.cancel),
          ),
          ElevatedButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: Text(l10n.mcpUninstall),
          ),
        ],
      ),
    );

    if (confirmed == true) {
      await widget.repository.uninstallServer(server.id);
      if (!mounted) return;
      await _loadServers();
      if (!mounted) return;
      showOpenChatToast(
        context,
        l10n.mcpDisabledSuccess,
        type: OpenChatToastType.info,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;

    final categories = [
      'All',
      'Developer',
      'Database',
      'Web & Search',
      'DevOps & Cloud',
      'Productivity',
      'Memory & Reasoning',
      'Finance',
    ];

    final filteredServers = _servers.where((server) {
      if (_selectedCategory != 'All' &&
          server.category != _selectedCategory) {
        return false;
      }
      if (_searchQuery.isNotEmpty) {
        final q = _searchQuery.toLowerCase();
        final matchName = server.name.toLowerCase().contains(q);
        final matchDesc = server.description.toLowerCase().contains(q);
        final matchCategory = server.category.toLowerCase().contains(q);
        final matchTools = server.discoveredTools.any(
          (t) => t.toLowerCase().contains(q),
        );
        return matchName || matchDesc || matchCategory || matchTools;
      }
      return true;
    }).toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        // Search bar
        TextField(
          controller: _searchController,
          onChanged: (val) => setState(() => _searchQuery = val.trim()),
          decoration: InputDecoration(
            labelText: l10n.mcpSearchHint,
            prefixIcon: const Icon(LucideIcons.search),
            suffixIcon: _searchQuery.isNotEmpty
                ? IconButton(
                    icon: const Icon(LucideIcons.x),
                    onPressed: () {
                      _searchController.clear();
                      setState(() => _searchQuery = '');
                    },
                  )
                : null,
          ),
        ),
        const SizedBox(height: 16),

        // Category filter chips
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(
            children: categories.map((cat) {
              final isSelected = _selectedCategory == cat;
              return Padding(
                padding: const EdgeInsets.only(right: 8),
                child: FilterChip(
                  label: Text(cat),
                  selected: isSelected,
                  onSelected: (selected) {
                    if (selected) {
                      setState(() => _selectedCategory = cat);
                    }
                  },
                ),
              );
            }).toList(),
          ),
        ),
        const SizedBox(height: 20),

        // Server list or loading
        if (_isLoading)
          const Center(
            child: Padding(
              padding: EdgeInsets.all(48),
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
          )
        else if (filteredServers.isEmpty)
          Center(
            child: Padding(
              padding: const EdgeInsets.all(32),
              child: Text(
                'Aramayla eşleşen MCP aracı bulunamadı.',
                style: TextStyle(
                  fontSize: OpenChatTypography.body,
                  fontFamily: OpenChatTypography.uiFontFamily,
                  color: palette.secondaryText,
                ),
              ),
            ),
          )
        else
          ListView.separated(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            itemCount: filteredServers.length,
            separatorBuilder: (_, _) => const SizedBox(height: 16),
            itemBuilder: (context, index) {
              final server = filteredServers[index];
              return _buildServerCard(context, palette, l10n, server);
            },
          ),
      ],
    );
  }

  Widget _buildServerCard(
    BuildContext context,
    OpenChatPalette palette,
    AppLocalizations l10n,
    GlobalMcpServer server,
  ) {
    final hasEnv = server.requiredEnvKeys.isNotEmpty ||
        server.optionalEnvKeys.isNotEmpty;
    final isProbing = _probingServerIds.contains(server.id);

    return ChatSurfaceCard(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: palette.composer,
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(
                      color: palette.border.withValues(alpha: 0.1),
                    ),
                  ),
                  child: Icon(
                    LucideIcons.blocks,
                    color: palette.accentIcon,
                    size: 20,
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Text(
                            server.name,
                            style: TextStyle(
                              fontSize: OpenChatTypography.componentTitle,
                              fontFamily: OpenChatTypography.uiFontFamily,
                              fontWeight: FontWeight.w600,
                              color: palette.text,
                            ),
                          ),
                          const SizedBox(width: 8),
                          Container(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 6,
                              vertical: 2,
                            ),
                            decoration: BoxDecoration(
                              color: palette.selected,
                              borderRadius: BorderRadius.circular(4),
                            ),
                            child: Text(
                              server.category,
                              style: TextStyle(
                                fontSize: OpenChatTypography.metadata,
                                fontFamily: OpenChatTypography.uiFontFamily,
                                color: palette.secondaryText,
                              ),
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 2),
                      Text(
                        '${server.command} ${server.args.join(' ')}',
                        style: TextStyle(
                          fontSize: OpenChatTypography.code,
                          fontFamily: OpenChatTypography.codeFontFamily,
                          color: palette.secondaryText,
                        ),
                      ),
                    ],
                  ),
                ),
                if (!server.installed)
                  ElevatedButton.icon(
                    onPressed: () => _installServer(server),
                    icon: const Icon(LucideIcons.downloadCloud, size: 16),
                    label: Text(l10n.mcpInstall),
                  )
                else
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        server.enabled ? l10n.mcpActive : l10n.mcpInactive,
                        style: TextStyle(
                          fontSize: OpenChatTypography.metadata,
                          fontFamily: OpenChatTypography.uiFontFamily,
                          fontWeight: FontWeight.w600,
                          color: server.enabled
                              ? palette.accent
                              : palette.secondaryText,
                        ),
                      ),
                      const SizedBox(width: 8),
                      Switch(
                        value: server.enabled,
                        onChanged: (val) => _toggleEnabled(server, val),
                      ),
                    ],
                  ),
              ],
            ),
            const SizedBox(height: 12),
            Text(
              server.description,
              style: TextStyle(
                fontSize: OpenChatTypography.body,
                fontFamily: OpenChatTypography.uiFontFamily,
                color: palette.text,
              ),
            ),
            const SizedBox(height: 12),

            // Discovered Tools Section
            if (server.installed) ...[
              if (server.discoveredTools.isNotEmpty) ...[
                Row(
                  children: [
                    Text(
                      'Otomatik Çekilen Araç Tanımları (${server.discoveredTools.length}):',
                      style: TextStyle(
                        fontSize: OpenChatTypography.metadata,
                        fontFamily: OpenChatTypography.uiFontFamily,
                        color: palette.secondaryText,
                      ),
                    ),
                    const SizedBox(width: 8),
                    if (isProbing)
                      const SizedBox(
                        width: 12,
                        height: 12,
                        child: CircularProgressIndicator(strokeWidth: 1.5),
                      )
                    else
                      InkWell(
                        onTap: () => _probeServer(server),
                        child: Padding(
                          padding: const EdgeInsets.all(2),
                          child: Icon(
                            LucideIcons.refreshCw,
                            size: 13,
                            color: palette.accent,
                          ),
                        ),
                      ),
                  ],
                ),
                const SizedBox(height: 6),
                Wrap(
                  spacing: 6,
                  runSpacing: 6,
                  children: server.discoveredTools.map((tool) {
                    return Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 8,
                        vertical: 4,
                      ),
                      decoration: BoxDecoration(
                        color: palette.composer,
                        borderRadius: BorderRadius.circular(6),
                        border: Border.all(
                          color: palette.border.withValues(alpha: 0.1),
                        ),
                      ),
                      child: Text(
                        tool,
                        style: TextStyle(
                          fontSize: OpenChatTypography.code,
                          fontFamily: OpenChatTypography.codeFontFamily,
                          color: palette.text,
                        ),
                      ),
                    );
                  }).toList(),
                ),
                const SizedBox(height: 12),
              ] else ...[
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 8,
                  ),
                  decoration: BoxDecoration(
                    color: palette.composer.withValues(alpha: 0.5),
                    borderRadius: BorderRadius.circular(6),
                    border: Border.all(
                      color: palette.border.withValues(alpha: 0.1),
                    ),
                  ),
                  child: Row(
                    children: [
                      Icon(
                        LucideIcons.info,
                        size: 15,
                        color: palette.secondaryIcon,
                      ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          l10n.mcpNoToolsDiscovered,
                          style: TextStyle(
                            fontSize: OpenChatTypography.metadata,
                            fontFamily: OpenChatTypography.uiFontFamily,
                            color: palette.secondaryText,
                          ),
                        ),
                      ),
                      OutlinedButton.icon(
                        onPressed: isProbing ? null : () => _probeServer(server),
                        icon: isProbing
                            ? const SizedBox(
                                width: 12,
                                height: 12,
                                child: CircularProgressIndicator(strokeWidth: 1.5),
                              )
                            : const Icon(LucideIcons.refreshCw, size: 13),
                        label: Text(
                          isProbing ? l10n.mcpDiscovering : l10n.mcpDiscoverTools,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 12),
              ],
            ],

            // Action Buttons
            if (server.installed)
              Row(
                children: [
                  if (hasEnv) ...[
                    OutlinedButton.icon(
                      onPressed: () => McpEnvDialog.show(
                        context: context,
                        server: server,
                        repository: widget.repository,
                      ),
                      icon: const Icon(LucideIcons.key, size: 15),
                      label: Text(l10n.mcpConfigureEnv),
                    ),
                    const SizedBox(width: 8),
                  ],
                  if (server.discoveredTools.isNotEmpty) ...[
                    OutlinedButton.icon(
                      onPressed: isProbing ? null : () => _probeServer(server),
                      icon: isProbing
                          ? const SizedBox(
                              width: 12,
                              height: 12,
                              child: CircularProgressIndicator(strokeWidth: 1.5),
                            )
                          : const Icon(LucideIcons.refreshCw, size: 14),
                      label: Text(
                        isProbing ? l10n.mcpDiscovering : l10n.mcpDiscoverTools,
                      ),
                    ),
                    const SizedBox(width: 8),
                  ],
                  TextButton.icon(
                    onPressed: () => _uninstallServer(server),
                    icon: Icon(
                      LucideIcons.trash2,
                      size: 15,
                      color: palette.destructive,
                    ),
                    label: Text(
                      l10n.mcpUninstall,
                      style: TextStyle(color: palette.destructive),
                    ),
                  ),
                  const Spacer(),
                  Text(
                    'Konum: %LOCALAPPDATA%\\OpenChat\\mcp\\${server.id}.json',
                    style: TextStyle(
                      fontSize: OpenChatTypography.metadata,
                      fontFamily: OpenChatTypography.uiFontFamily,
                      color: palette.secondaryText,
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
