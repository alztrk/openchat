import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/domain/title_generation_preference.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ChatGptTitlePreferenceSection extends StatelessWidget {
  const ChatGptTitlePreferenceSection({
    required this.preference,
    required this.models,
    required this.palette,
    required this.isLoading,
    required this.hasError,
    required this.isSaving,
    required this.onRetry,
    required this.onModeChanged,
    required this.onModelChanged,
    super.key,
  });

  final TitleGenerationPreference preference;
  final List<ChatGptModel> models;
  final OpenChatPalette palette;
  final bool isLoading;
  final bool hasError;
  final bool isSaving;
  final VoidCallback onRetry;
  final ValueChanged<String?> onModeChanged;
  final ValueChanged<String?> onModelChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final selectedModel = _selectedModel;
    final selectedModelRouteKey = preference.model?.routeKey;
    final lightweightAvailable = models.any(
      (model) => _parameterCountBillions(model) != null,
    );
    final modelOptions = <OpenChatSelectOption<String>>[
      for (final model in models)
        OpenChatSelectOption<String>(
          value: model.routeKey,
          label:
              '${model.sourceLabel ?? _providerLabel(l10n, model.providerId)} · ${model.displayName}',
        ),
      if (selectedModelRouteKey != null && selectedModel == null)
        OpenChatSelectOption<String>(
          value: selectedModelRouteKey,
          enabled: false,
          label: preference.model!.displayName,
        ),
    ];

    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (isLoading)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 12),
              child: Row(
                children: [
                  SizedBox.square(
                    dimension: 16,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: palette.accent,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Text(
                    l10n.modelsLoading,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 13,
                    ),
                  ),
                ],
              ),
            )
          else if (hasError)
            _LoadError(palette: palette, onRetry: onRetry)
          else ...[
            _TitleModeOption(
              icon: LucideIcons.messagesSquare,
              title: l10n.titleUseConversationModel,
              description: l10n.titleConversationModeDescription,
              selected: preference.mode == TitleGenerationMode.conversation,
              enabled: !isSaving,
              palette: palette,
              onTap: () => onModeChanged(TitleGenerationMode.conversation.name),
            ),
            const SizedBox(height: 8),
            _TitleModeOption(
              icon: LucideIcons.listFilter,
              title: l10n.titleUseSpecificModel,
              description: l10n.titleSpecificModelDescription,
              selected: preference.mode == TitleGenerationMode.selectedModel,
              enabled: !isSaving && models.isNotEmpty,
              palette: palette,
              onTap: () =>
                  onModeChanged(TitleGenerationMode.selectedModel.name),
            ),
            const SizedBox(height: 8),
            _TitleModeOption(
              icon: LucideIcons.sparkles,
              title: l10n.titleUseLightweightModel,
              description: l10n.titleLightweightModeDescription,
              selected: preference.mode == TitleGenerationMode.lightweight,
              enabled: !isSaving && lightweightAvailable,
              palette: palette,
              onTap: () => onModeChanged(TitleGenerationMode.lightweight.name),
            ),
            if (preference.mode == TitleGenerationMode.selectedModel) ...[
              const SizedBox(height: 12),
              Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: palette.raisedSurface,
                  borderRadius: BorderRadius.circular(OpenChatRadii.button),
                  border: Border.all(color: palette.subtleBorder),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Text(
                      l10n.titleSelectModelHint,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 8),
                    LayoutBuilder(
                      builder: (context, constraints) => OpenChatSelect<String>(
                        options: modelOptions,
                        value: selectedModelRouteKey,
                        onChanged: isSaving || models.isEmpty
                            ? null
                            : onModelChanged,
                        palette: palette,
                        width: double.infinity,
                        menuWidth: constraints.maxWidth,
                        height: 42,
                        hint: l10n.titleSelectModelHint,
                        selectedContent: selectedModel == null
                            ? null
                            : Row(
                                children: [
                                  ProviderIcon(
                                    providerId:
                                        selectedModel.providerId ==
                                            'chatgpt_api'
                                        ? 'chatgpt'
                                        : selectedModel.providerId,
                                    color: palette.secondaryIcon,
                                    size: 16,
                                  ),
                                  const SizedBox(width: 8),
                                  Expanded(
                                    child: Text(
                                      '${selectedModel.sourceLabel ?? _providerLabel(l10n, selectedModel.providerId)} · ${selectedModel.displayName}',
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: TextStyle(
                                        color: palette.text,
                                        fontSize: 13,
                                        fontWeight: FontWeight.w500,
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
            if (preference.mode == TitleGenerationMode.lightweight) ...[
              const SizedBox(height: 10),
              _LightweightStatus(
                modelName: preference.model?.displayName,
                palette: palette,
              ),
            ],
            if (isSaving) ...[
              const SizedBox(height: 12),
              ClipRRect(
                borderRadius: BorderRadius.circular(2),
                child: LinearProgressIndicator(
                  minHeight: 2,
                  color: palette.accent,
                  backgroundColor: palette.subtleBorder,
                ),
              ),
            ],
          ],
        ],
      ),
    );
  }

  ChatGptModel? get _selectedModel {
    final routeKey = preference.model?.routeKey;
    if (routeKey == null) return null;
    for (final model in models) {
      if (model.routeKey == routeKey) return model;
    }
    return null;
  }
}

class _TitleModeOption extends StatelessWidget {
  const _TitleModeOption({
    required this.icon,
    required this.title,
    required this.description,
    required this.selected,
    required this.enabled,
    required this.palette,
    required this.onTap,
  });

  final IconData icon;
  final String title;
  final String description;
  final bool selected;
  final bool enabled;
  final OpenChatPalette palette;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final foreground = enabled ? palette.text : palette.disabledForeground;
    final iconColor = enabled
        ? selected
              ? palette.accentIcon
              : palette.secondaryIcon
        : palette.disabledIcon;

    return Semantics(
      button: true,
      enabled: enabled,
      checked: selected,
      inMutuallyExclusiveGroup: true,
      label: '$title. $description',
      child: Material(
        color: selected ? palette.selected : Colors.transparent,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.button),
          side: BorderSide(
            color: selected
                ? palette.accent.withValues(alpha: 0.7)
                : Colors.transparent,
          ),
        ),
        child: InkWell(
          onTap: enabled ? onTap : null,
          borderRadius: BorderRadius.circular(OpenChatRadii.button),
          child: Padding(
            padding: const EdgeInsetsDirectional.fromSTEB(12, 11, 12, 11),
            child: Row(
              children: [
                Icon(icon, size: 18, color: iconColor),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        title,
                        style: TextStyle(
                          color: foreground,
                          fontSize: 13,
                          fontWeight: selected
                              ? FontWeight.w600
                              : FontWeight.w500,
                          height: 1.25,
                        ),
                      ),
                      const SizedBox(height: 3),
                      Text(
                        description,
                        style: TextStyle(
                          color: enabled
                              ? palette.secondaryText
                              : palette.disabledForeground,
                          fontSize: 12,
                          height: 1.35,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 10),
                Icon(
                  selected ? LucideIcons.circleCheck : LucideIcons.circle,
                  size: 18,
                  color: selected ? palette.accentIcon : palette.secondaryIcon,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _LightweightStatus extends StatelessWidget {
  const _LightweightStatus({required this.modelName, required this.palette});

  final String? modelName;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final hasModel = modelName != null;
    return Container(
      padding: const EdgeInsetsDirectional.fromSTEB(12, 10, 12, 10),
      decoration: BoxDecoration(
        color: hasModel
            ? palette.success.withValues(alpha: 0.08)
            : palette.warning.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(OpenChatRadii.button),
        border: Border.all(
          color: hasModel
              ? palette.success.withValues(alpha: 0.25)
              : palette.warning.withValues(alpha: 0.25),
        ),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            hasModel ? LucideIcons.circleCheck : LucideIcons.circleAlert,
            size: 16,
            color: hasModel ? palette.success : palette.warning,
          ),
          const SizedBox(width: 9),
          Expanded(
            child: Text(
              hasModel
                  ? l10n.titleLightweightModelSelected(modelName!)
                  : l10n.titleLightweightUnavailable,
              style: TextStyle(
                color: palette.secondaryText,
                fontSize: 12,
                height: 1.4,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _LoadError extends StatelessWidget {
  const _LoadError({required this.palette, required this.onRetry});

  final OpenChatPalette palette;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return Row(
      children: [
        Expanded(
          child: Text(
            l10n.titlePreferenceLoadFailed,
            style: TextStyle(color: palette.secondaryText, fontSize: 13),
          ),
        ),
        TextButton.icon(
          onPressed: onRetry,
          icon: const Icon(LucideIcons.refreshCw, size: 16),
          label: Text(l10n.retry),
        ),
      ],
    );
  }
}

String _providerLabel(AppLocalizations l10n, String providerId) =>
    switch (providerId) {
      'chatgpt' || 'chatgpt_api' => l10n.chatGptProvider,
      'opencode' => l10n.openCodeProvider,
      'gemini' => l10n.geminiProvider,
      'groq' => l10n.groqProvider,
      'cerebras' => l10n.cerebrasProvider,
      'openrouter' => l10n.openRouterProvider,
      'mistral' => l10n.mistralProvider,
      'llama_cpp' => 'llama.cpp',
      'vllm' => 'vLLM',
      'exllama' => 'ExLlamaV3',
      final customId when customId.startsWith('custom_') =>
        l10n.customProvidersHeading,
      _ => providerId,
    };

String? _parameterCountBillions(ChatGptModel model) {
  final pattern = RegExp(
    r'(?:^|[-_:./\s])(\d+(?:\.\d+)?)b(?:$|[-_:./\s])',
    caseSensitive: false,
  );
  for (final value in [model.id, model.displayName]) {
    final match = pattern.firstMatch(value);
    final count = double.tryParse(match?.group(1) ?? '');
    if (count != null && count > 0) return match?.group(1);
  }
  return null;
}
