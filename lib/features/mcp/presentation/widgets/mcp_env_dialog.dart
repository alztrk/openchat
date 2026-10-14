import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/mcp/data/global_mcp_repository.dart';
import 'package:openchat/features/mcp/domain/global_mcp_server.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class McpEnvDialog extends StatefulWidget {
  const McpEnvDialog({
    required this.server,
    required this.repository,
    super.key,
  });

  final GlobalMcpServer server;
  final GlobalMcpRepository repository;

  static Future<void> show({
    required BuildContext context,
    required GlobalMcpServer server,
    required GlobalMcpRepository repository,
  }) {
    return showDialog<void>(
      context: context,
      builder: (context) => McpEnvDialog(
        server: server,
        repository: repository,
      ),
    );
  }

  @override
  State<McpEnvDialog> createState() => _McpEnvDialogState();
}

class _McpEnvDialogState extends State<McpEnvDialog> {
  final Map<String, TextEditingController> _controllers = {};
  final Map<String, bool> _obscureFields = {};
  bool _isLoading = true;
  bool _isSaving = false;

  @override
  void initState() {
    super.initState();
    _loadEnvValues();
  }

  @override
  void dispose() {
    for (final controller in _controllers.values) {
      controller.dispose();
    }
    super.dispose();
  }

  Future<void> _loadEnvValues() async {
    final allKeys = [
      ...widget.server.requiredEnvKeys,
      ...widget.server.optionalEnvKeys,
    ];

    for (final key in allKeys) {
      final value = await widget.repository.readSecureEnv(
        widget.server.id,
        key,
      );
      _controllers[key] = TextEditingController(text: value ?? '');
      _obscureFields[key] = true;
    }

    if (mounted) {
      setState(() => _isLoading = false);
    }
  }

  Future<void> _saveEnvValues() async {
    setState(() => _isSaving = true);
    try {
      for (final entry in _controllers.entries) {
        await widget.repository.writeSecureEnv(
          widget.server.id,
          entry.key,
          entry.value.text,
        );
      }
      if (mounted) {
        Navigator.of(context).pop();
        showOpenChatToast(
          context,
          context.openchatL10n.mcpEnvSaveSuccess,
          type: OpenChatToastType.success,
        );
      }
    } finally {
      if (mounted) {
        setState(() => _isSaving = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final allKeys = [
      ...widget.server.requiredEnvKeys,
      ...widget.server.optionalEnvKeys,
    ];

    return Dialog(
      backgroundColor: palette.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(OpenChatRadii.dialog),
        side: BorderSide(color: palette.border),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 540),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Icon(
                    LucideIcons.key,
                    color: palette.accentIcon,
                    size: 22,
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(
                      '${widget.server.name} - ${l10n.mcpEnvDialogTitle}',
                      style: TextStyle(
                        fontSize: OpenChatTypography.componentTitle,
                        fontFamily: OpenChatTypography.uiFontFamily,
                        fontWeight: FontWeight.w600,
                        color: palette.text,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Text(
                l10n.mcpEnvDialogDescription,
                style: TextStyle(
                  fontSize: OpenChatTypography.metadata,
                  fontFamily: OpenChatTypography.uiFontFamily,
                  color: palette.secondaryText,
                ),
              ),
              const SizedBox(height: 20),
              if (_isLoading)
                const Center(
                  child: Padding(
                    padding: EdgeInsets.all(32),
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                )
              else if (allKeys.isEmpty)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 16),
                  child: Text(
                    'Bu MCP aracı için herhangi bir ortam değişkeni (API anahtarı vb.) gerekmemektedir.',
                    style: TextStyle(
                      fontSize: OpenChatTypography.body,
                      fontFamily: OpenChatTypography.uiFontFamily,
                      color: palette.secondaryText,
                    ),
                  ),
                )
              else
                Flexible(
                  child: SingleChildScrollView(
                    child: Column(
                      children: allKeys.map((key) {
                        final isRequired =
                            widget.server.requiredEnvKeys.contains(key);
                        final controller = _controllers[key]!;
                        final isObscured = _obscureFields[key] ?? true;

                        return Padding(
                          padding: const EdgeInsets.only(bottom: 16),
                          child: TextField(
                            controller: controller,
                            obscureText: isObscured,
                            decoration: InputDecoration(
                              labelText: isRequired ? '$key *' : key,
                              hintText: '$key değerini girin',
                              suffixIcon: IconButton(
                                icon: Icon(
                                  isObscured
                                      ? LucideIcons.eye
                                      : LucideIcons.eyeOff,
                                  size: 18,
                                ),
                                onPressed: () {
                                  setState(() {
                                    _obscureFields[key] = !isObscured;
                                  });
                                },
                              ),
                            ),
                          ),
                        );
                      }).toList(),
                    ),
                  ),
                ),
              const SizedBox(height: 20),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: Text(l10n.cancel),
                  ),
                  const SizedBox(width: 12),
                  ElevatedButton(
                    onPressed: _isSaving ? null : _saveEnvValues,
                    child: _isSaving
                        ? const SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : Text(l10n.save),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
