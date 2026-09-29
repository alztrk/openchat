import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ChatGptApiKeyForm extends StatefulWidget {
  const ChatGptApiKeyForm({
    required this.controller,
    required this.errorText,
    required this.isSaving,
    required this.onChanged,
    required this.onSave,
    required this.onCancel,
    super.key,
  });

  final TextEditingController controller;
  final String? errorText;
  final bool isSaving;
  final VoidCallback onChanged;
  final VoidCallback onSave;
  final VoidCallback onCancel;

  @override
  State<ChatGptApiKeyForm> createState() => _ChatGptApiKeyFormState();
}

class _ChatGptApiKeyFormState extends State<ChatGptApiKeyForm> {
  bool _showApiKey = false;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Card(
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
              TextField(
                controller: widget.controller,
                autofocus: true,
                autocorrect: false,
                enableSuggestions: false,
                obscureText: !_showApiKey,
                keyboardType: TextInputType.visiblePassword,
                textInputAction: TextInputAction.done,
                onChanged: (_) => widget.onChanged(),
                onSubmitted: (_) => widget.onSave(),
                decoration: InputDecoration(
                  labelText: l10n.apiKeyInputLabel,
                  errorText: widget.errorText,
                  suffixIcon: IconButton(
                    tooltip: _showApiKey ? l10n.hideApiKey : l10n.showApiKey,
                    onPressed: () => setState(() => _showApiKey = !_showApiKey),
                    icon: Icon(
                      _showApiKey
                          ? Icons.visibility_off_outlined
                          : Icons.visibility_outlined,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 12),
              Wrap(
                alignment: WrapAlignment.end,
                spacing: 8,
                runSpacing: 8,
                children: [
                  TextButton(
                    onPressed: widget.isSaving ? null : widget.onCancel,
                    child: Text(l10n.cancel),
                  ),
                  FilledButton.icon(
                    onPressed:
                        widget.isSaving || widget.controller.text.trim().isEmpty
                        ? null
                        : () => widget.onSave(),
                    icon: widget.isSaving
                        ? const SizedBox.square(
                            dimension: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.save_outlined, size: 16),
                    label: Text(widget.isSaving ? l10n.saving : l10n.save),
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
