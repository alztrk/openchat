import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:shadcn_ui/shadcn_ui.dart' as shad;

import 'package:openchat/app/openchat_glass_surface.dart';
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
  late final FocusNode _apiKeyFocusNode;
  bool _isApiKeyFocused = false;

  @override
  void initState() {
    super.initState();
    _apiKeyFocusNode = FocusNode(debugLabel: 'ChatGPT API key');
    _apiKeyFocusNode.addListener(_handleApiKeyFocusChanged);
  }

  void _handleApiKeyFocusChanged() {
    if (!mounted || _isApiKeyFocused == _apiKeyFocusNode.hasFocus) return;
    setState(() => _isApiKeyFocused = _apiKeyFocusNode.hasFocus);
  }

  @override
  void dispose() {
    _apiKeyFocusNode
      ..removeListener(_handleApiKeyFocusChanged)
      ..dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final theme = Theme.of(context);
    final errorText = widget.errorText;
    final inputRadius = BorderRadius.circular(OpenChatRadii.control);
    final reducedMotion = MediaQuery.disableAnimationsOf(context);
    final motionDuration = reducedMotion ? Duration.zero : OpenChatMotion.quick;

    return Padding(
      padding: const EdgeInsets.only(top: OpenChatSpacing.sm),
      child: OpenChatGlassSurface(
        borderRadius: BorderRadius.circular(OpenChatRadii.card),
        padding: const EdgeInsets.all(OpenChatSpacing.md),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            ExcludeSemantics(
              child: Text(
                l10n.apiKeyInputLabel,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: palette.secondaryText,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            const SizedBox(height: OpenChatSpacing.xs),
            Semantics(
              textField: true,
              label: l10n.apiKeyInputLabel,
              child: shad.ShadInput(
                focusNode: _apiKeyFocusNode,
                controller: widget.controller,
                autofocus: true,
                autocorrect: false,
                enableSuggestions: false,
                obscureText: !_showApiKey,
                keyboardType: TextInputType.visiblePassword,
                textInputAction: TextInputAction.done,
                onChanged: (_) => widget.onChanged(),
                onSubmitted: (_) => widget.onSave(),
                cursorColor: palette.focusRing,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: palette.text,
                ),
                placeholderStyle: theme.textTheme.bodyMedium?.copyWith(
                  color: palette.secondaryText,
                ),
                trailing: IconButton(
                  tooltip: _showApiKey ? l10n.hideApiKey : l10n.showApiKey,
                  onPressed: () => setState(() => _showApiKey = !_showApiKey),
                  icon: Icon(
                    _showApiKey ? LucideIcons.eyeOff : LucideIcons.eye,
                  ),
                ),
                padding: const EdgeInsets.symmetric(
                  horizontal: OpenChatSpacing.sm,
                  vertical: OpenChatSpacing.xs,
                ),
                constraints: const BoxConstraints(minHeight: 40),
                decoration: shad.ShadDecoration(
                  color: palette.surface,
                  hasError: errorText != null,
                  border: shad.ShadBorder.all(
                    color: palette.controlBorder,
                    radius: inputRadius,
                  ),
                  focusedBorder: shad.ShadBorder.all(
                    color: palette.focusRing,
                    width: 2,
                    radius: inputRadius,
                  ),
                  errorBorder: shad.ShadBorder.all(
                    color: palette.destructive,
                    radius: inputRadius,
                  ),
                  secondaryErrorBorder: errorText != null && _isApiKeyFocused
                      ? shad.ShadBorder.all(
                          color: palette.focusRing,
                          width: 2,
                          radius: inputRadius,
                        )
                      : null,
                ),
                bottom: Semantics(
                  liveRegion: errorText != null,
                  label: errorText,
                  child: ExcludeSemantics(
                    child: AnimatedSize(
                      duration: motionDuration,
                      curve: Curves.easeOutCubic,
                      alignment: Alignment.topCenter,
                      child: AnimatedSwitcher(
                        duration: motionDuration,
                        child: errorText == null
                            ? const SizedBox.shrink(
                                key: ValueKey<String>('api-key-error-empty'),
                              )
                            : Padding(
                                key: ValueKey<String>(errorText),
                                padding: const EdgeInsets.only(
                                  top: OpenChatSpacing.xs,
                                ),
                                child: Text(
                                  errorText,
                                  style: theme.textTheme.bodySmall?.copyWith(
                                    color: palette.destructive,
                                  ),
                                ),
                              ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
            const SizedBox(height: OpenChatSpacing.sm),
            Wrap(
              alignment: WrapAlignment.end,
              spacing: OpenChatSpacing.xs,
              runSpacing: OpenChatSpacing.xs,
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
                      ? reducedMotion
                            ? const Icon(LucideIcons.loaderCircle, size: 16)
                            : const SizedBox.square(
                                dimension: 16,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                      : const Icon(LucideIcons.save, size: 16),
                  label: Text(widget.isSaving ? l10n.saving : l10n.save),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
