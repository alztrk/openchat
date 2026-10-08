import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/app/openchat_page_header.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';
import 'package:openchat/features/chat/data/conversation_memory_repository.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_memory_dialog.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/features/settings/data/chat_gpt_api_key_store.dart';
import 'package:openchat/features/settings/data/open_code_api_key_store.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

import 'package:openchat/features/settings/presentation/chat_gpt_connection_section.dart';
import 'package:openchat/features/settings/presentation/compatible_provider_connection_section.dart';
import 'package:openchat/features/settings/presentation/conversation_archive_section.dart';
import 'package:openchat/features/settings/presentation/local_engines_settings_section.dart';
import 'package:openchat/features/settings/presentation/models_settings_section.dart';
import 'package:openchat/features/settings/presentation/open_code_connection_section.dart';
import 'package:openchat/features/settings/presentation/profile_archive_section.dart';
import 'package:openchat/features/settings/presentation/statistics_settings_section.dart';
import 'package:openchat/features/settings/presentation/usage_quotas_settings_section.dart';

const _sharedInstructionsMaxLength = 4096;

enum _SettingsSection {
  connections,
  usageQuotas,
  statistics,
  models,
  localEngines,
  conversationMemory,
  sharedInstructions,
  appearance,
  localData;

  bool get isAdvanced => switch (this) {
    usageQuotas ||
    statistics ||
    models ||
    localEngines ||
    conversationMemory => true,
    connections || sharedInstructions || appearance || localData => false,
  };

  IconData get icon => switch (this) {
    connections => LucideIcons.link,
    usageQuotas => LucideIcons.chartNoAxesCombined,
    statistics => LucideIcons.chartBarIncreasing,
    models => LucideIcons.slidersHorizontal,
    localEngines => LucideIcons.microchip,
    conversationMemory => LucideIcons.brain,
    sharedInstructions => LucideIcons.notebookPen,
    appearance => LucideIcons.palette,
    localData => LucideIcons.database,
  };
}

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    required this.themeMode,
    required this.onThemeModeChanged,
    required this.historyStorageStatus,
    required this.hasConversationHistory,
    this.activeConversationId,
    this.activeConversationTitle,
    this.isActiveConversationSending = false,
    required this.settingsPreferences,
    this.pageHeadingFocusNode,
    this.locale,
    this.conversationWidth = ConversationWidthPreference.normal,
    this.conversationTextSize = ConversationTextSizePreference.normal,
    this.appFont = AppFontPreference.sourceSans3,
    this.onLocaleChanged,
    this.onConversationWidthChanged,
    this.onConversationTextSizeChanged,
    this.onAppFontChanged,
    this.onClearConversationHistory,
    this.chatGptApiKeyStore,
    this.apiCompatibleProviderKeyStore,
    this.openCodeApiKeyStore,
    this.serviceClient,
    this.chatRepository,
    this.onProviderStateChanged,
    this.onConnectionRemoved,
    this.onOpenConversation,
    super.key,
  });

  final ThemeMode themeMode;
  final Locale? locale;
  final ConversationWidthPreference conversationWidth;
  final ConversationTextSizePreference conversationTextSize;
  final AppFontPreference appFont;
  final Future<void> Function(ThemeMode) onThemeModeChanged;
  final Future<void> Function(Locale?)? onLocaleChanged;
  final Future<void> Function(ConversationWidthPreference)?
  onConversationWidthChanged;
  final Future<void> Function(ConversationTextSizePreference)?
  onConversationTextSizeChanged;
  final Future<void> Function(AppFontPreference)? onAppFontChanged;
  final HistoryStorageStatus historyStorageStatus;
  final bool hasConversationHistory;
  final String? activeConversationId;
  final String? activeConversationTitle;
  final bool isActiveConversationSending;
  final SettingsPreferences settingsPreferences;
  final FocusNode? pageHeadingFocusNode;
  final Future<void> Function()? onClearConversationHistory;
  final ChatGptApiKeyStore? chatGptApiKeyStore;
  final ApiCompatibleProviderKeyStore? apiCompatibleProviderKeyStore;
  final OpenCodeApiKeyStore? openCodeApiKeyStore;
  final OpenChatServiceClient? serviceClient;
  final ChatRepository? chatRepository;
  final Future<void> Function()? onProviderStateChanged;
  final Future<void> Function(String connectionId)? onConnectionRemoved;
  final ValueChanged<String>? onOpenConversation;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  late final TextEditingController _sharedInstructionsController;
  bool _isClearingHistory = false;
  bool _isSavingLanguage = false;
  bool _isSavingAppearancePreference = false;
  bool _isLoadingInstructions = true;
  bool _isSavingInstructions = false;
  String _savedSharedInstructions = '';
  String? _sharedInstructionsError;
  _SettingsSection _selectedSection = _SettingsSection.connections;
  bool _advancedSettingsExpanded = false;
  ConversationMemoryRepository? _conversationMemoryRepository;

  @override
  void initState() {
    super.initState();
    _sharedInstructionsController = TextEditingController();
    final serviceClient = widget.serviceClient;
    _conversationMemoryRepository = serviceClient == null
        ? null
        : ConversationMemoryRepository(serviceClient);
    unawaited(_loadSharedInstructions());
  }

  @override
  void didUpdateWidget(covariant SettingsScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.serviceClient != widget.serviceClient) {
      final serviceClient = widget.serviceClient;
      _conversationMemoryRepository = serviceClient == null
          ? null
          : ConversationMemoryRepository(serviceClient);
    }
  }

  @override
  void dispose() {
    _sharedInstructionsController.dispose();
    super.dispose();
  }

  void _selectSection(_SettingsSection section) {
    setState(() {
      _selectedSection = section;
      _advancedSettingsExpanded = section.isAdvanced;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) widget.pageHeadingFocusNode?.requestFocus();
    });
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;

    return LayoutBuilder(
      builder: (context, constraints) {
        final compactSidebar = constraints.maxWidth < 960;
        final horizontalInset = constraints.maxWidth < 640 ? 16.0 : 24.0;
        final sectionTitle = switch (_selectedSection) {
          _SettingsSection.connections => l10n.connections,
          _SettingsSection.usageQuotas => l10n.usageQuotas,
          _SettingsSection.statistics => l10n.statistics,
          _SettingsSection.models => l10n.modelPreferences,
          _SettingsSection.localEngines => l10n.localEngines,
          _SettingsSection.conversationMemory => l10n.conversationMemory,
          _SettingsSection.sharedInstructions => l10n.sharedInstructions,
          _SettingsSection.appearance => l10n.appearance,
          _SettingsSection.localData => l10n.localData,
        };
        final sectionDescription = switch (_selectedSection) {
          _SettingsSection.connections => l10n.settingsConnectionsDescription,
          _SettingsSection.usageQuotas => l10n.settingsUsageQuotasDescription,
          _SettingsSection.statistics => l10n.settingsStatisticsDescription,
          _SettingsSection.models => l10n.settingsModelPreferencesDescription,
          _SettingsSection.localEngines => l10n.settingsLocalEnginesDescription,
          _SettingsSection.conversationMemory =>
            l10n.settingsConversationMemoryDescription,
          _SettingsSection.sharedInstructions =>
            l10n.settingsSharedInstructionsDescription,
          _SettingsSection.appearance => l10n.settingsAppearanceDescription,
          _SettingsSection.localData => l10n.settingsLocalDataDescription,
        };

        return ColoredBox(
          color: palette.surface,
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _SettingsSidebar(
                compact: compactSidebar,
                selectedSection: _selectedSection,
                advancedExpanded: _advancedSettingsExpanded,
                onToggleAdvanced: () => setState(
                  () => _advancedSettingsExpanded = !_advancedSettingsExpanded,
                ),
                onSelectSection: _selectSection,
              ),
              Expanded(
                child: Column(
                  children: [
                    Padding(
                      padding: EdgeInsets.fromLTRB(
                        horizontalInset,
                        20,
                        horizontalInset,
                        16,
                      ),
                      child: OpenChatPageHeader(
                        title: sectionTitle,
                        description: sectionDescription,
                        focusNode: widget.pageHeadingFocusNode,
                      ),
                    ),
                    Expanded(
                      child: IndexedStack(
                        index: _selectedSection.index,
                        children: [
                          _buildSectionPage(
                            section: _SettingsSection.connections,
                            horizontalInset: horizontalInset,
                            child: _buildConnectionsSection(),
                          ),
                          _buildSectionPage(
                            section: _SettingsSection.usageQuotas,
                            horizontalInset: horizontalInset,
                            child: UsageQuotasSettingsSection(
                              serviceClient: widget.serviceClient,
                              onNavigateToConnections: () => setState(() {
                                _selectedSection = _SettingsSection.connections;
                                _advancedSettingsExpanded = false;
                              }),
                            ),
                          ),
                          _buildSectionPage(
                            section: _SettingsSection.statistics,
                            horizontalInset: horizontalInset,
                            child: StatisticsSettingsSection(
                              serviceClient: widget.serviceClient,
                              onOpenConversation: widget.onOpenConversation,
                            ),
                          ),
                          _buildSectionPage(
                            section: _SettingsSection.models,
                            horizontalInset: horizontalInset,
                            child: ModelsSettingsSection(
                              serviceClient: widget.serviceClient,
                              chatGptApiKeyStore: widget.chatGptApiKeyStore,
                              apiCompatibleProviderKeyStore:
                                  widget.apiCompatibleProviderKeyStore,
                              openCodeApiKeyStore: widget.openCodeApiKeyStore,
                              settingsPreferences: widget.settingsPreferences,
                              chatRepository: widget.chatRepository,
                              onChanged: widget.onProviderStateChanged,
                            ),
                          ),
                          _buildSectionPage(
                            section: _SettingsSection.localEngines,
                            horizontalInset: horizontalInset,
                            child: LocalEnginesSettingsSection(
                              serviceClient: widget.serviceClient,
                              settingsPreferences: widget.settingsPreferences,
                            ),
                          ),
                          _buildSectionPage(
                            section: _SettingsSection.conversationMemory,
                            horizontalInset: horizontalInset,
                            child: ConversationMemorySection(
                              repository: _conversationMemoryRepository,
                              conversationId: widget.activeConversationId,
                              conversationTitle: widget.activeConversationTitle,
                              isSending: widget.isActiveConversationSending,
                              embedded: true,
                            ),
                          ),
                          _buildSectionPage(
                            section: _SettingsSection.sharedInstructions,
                            horizontalInset: horizontalInset,
                            child: _buildSharedInstructionsSection(),
                          ),
                          _buildSectionPage(
                            section: _SettingsSection.appearance,
                            horizontalInset: horizontalInset,
                            child: _buildAppearanceSection(),
                          ),
                          _buildSectionPage(
                            section: _SettingsSection.localData,
                            horizontalInset: horizontalInset,
                            child: _buildLocalDataSection(),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildSectionPage({
    required _SettingsSection section,
    required double horizontalInset,
    required Widget child,
  }) {
    return SingleChildScrollView(
      key: PageStorageKey<_SettingsSection>(section),
      padding: EdgeInsets.fromLTRB(horizontalInset, 24, horizontalInset, 32),
      child: Align(
        alignment: Alignment.topLeft,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1040),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [child],
          ),
        ),
      ),
    );
  }

  Widget _buildConnectionsSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ChatGptConnectionSection(
          apiKeyStore: widget.chatGptApiKeyStore,
          serviceClient: widget.serviceClient,
          onProviderStateChanged: widget.onProviderStateChanged,
          onConnectionRemoved: widget.onConnectionRemoved,
        ),
        const SizedBox(height: 22),
        OpenCodeConnectionSection(
          apiKeyStore: widget.openCodeApiKeyStore,
          onChanged: widget.onProviderStateChanged,
        ),
        const SizedBox(height: 26),
        Text(
          context.openchatL10n.settingsOtherProviders,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 10),
        for (final providerId in ApiCompatibleProviderKeyStore.providerIds) ...[
          const SizedBox(height: 14),
          CompatibleProviderConnectionSection(
            providerId: providerId,
            apiKeyStore: widget.apiCompatibleProviderKeyStore,
            onChanged: widget.onProviderStateChanged,
          ),
        ],
      ],
    );
  }

  Widget _buildSharedInstructionsSection() {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          l10n.sharedInstructionsDescription,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 13,
            height: 1.45,
          ),
        ),
        const SizedBox(height: 14),
        if (_isLoadingInstructions)
          const LinearProgressIndicator()
        else if (_sharedInstructionsError != null)
          Row(
            children: [
              Expanded(
                child: Text(
                  _sharedInstructionsError!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ),
              TextButton.icon(
                onPressed: () => unawaited(_loadSharedInstructions()),
                icon: const Icon(LucideIcons.refreshCw),
                label: Text(l10n.retry),
              ),
            ],
          )
        else ...[
          Semantics(
            label: l10n.sharedInstructions,
            hint: l10n.sharedInstructionsDescription,
            child: TextField(
              controller: _sharedInstructionsController,
              enabled: !_isSavingInstructions,
              minLines: 4,
              maxLines: 8,
              maxLength: _sharedInstructionsMaxLength,
              buildCounter: (
                context, {
                required currentLength,
                required isFocused,
                maxLength,
              }) => null,
              style: Theme.of(context).textTheme.bodyLarge,
              cursorColor: palette.accent,
              textCapitalization: TextCapitalization.sentences,
              textAlignVertical: TextAlignVertical.top,
              decoration: InputDecoration(
                hintText: l10n.sharedInstructionsHint,
                hintStyle: TextStyle(
                  color: palette.secondaryText,
                  fontSize: 13,
                  height: 1.5,
                ),
                alignLabelWithHint: true,
                filled: true,
                fillColor: palette.composer,
                contentPadding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
              ),
              onChanged: (_) => setState(() {}),
            ),
          ),
          const SizedBox(height: 8),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            child: Wrap(
              alignment: WrapAlignment.spaceBetween,
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: 12,
              runSpacing: 12,
              children: [
                Text(
                  '${_sharedInstructionsController.text.characters.length}/$_sharedInstructionsMaxLength',
                  style: TextStyle(color: palette.secondaryText, fontSize: 12),
                ),
                FilledButton.icon(
                  onPressed:
                      _isSavingInstructions ||
                          _sharedInstructionsController.text ==
                              _savedSharedInstructions
                      ? null
                      : () => unawaited(_saveSharedInstructions()),
                  icon: _isSavingInstructions
                      ? const SizedBox.square(
                          dimension: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(LucideIcons.save),
                  label: Text(_isSavingInstructions ? l10n.saving : l10n.save),
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }

  Widget _buildAppearanceSection() {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final textTheme = Theme.of(context).textTheme;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _SettingsRow(
          title: l10n.theme,
          description: l10n.themeSettingDescription,
          textTheme: textTheme,
          controlWidth: 264,
          desktopControlTopInset: 0,
          controlBuilder: (width) => _SegmentedSelector<ThemeMode>(
            width: width,
            value: widget.themeMode,
            onChanged: (mode) => unawaited(_changeThemeMode(mode)),
            options: [
              (ThemeMode.system, l10n.systemTheme),
              (ThemeMode.light, l10n.lightTheme),
              (ThemeMode.dark, l10n.darkTheme),
            ],
            palette: palette,
          ),
        ),
        const SizedBox(height: 18),
        _SettingsRow(
          title: l10n.language,
          description: l10n.languageSettingDescription,
          textTheme: textTheme,
          controlWidth: 264,
          desktopControlTopInset: 0,
          controlBuilder: (width) {
            final languageCode = widget.locale?.languageCode ?? 'system';
            return OpenChatSelect<String>(
              options: [
                OpenChatSelectOption<String>(
                  value: 'system',
                  label: l10n.systemLanguage,
                ),
                OpenChatSelectOption<String>(
                  value: 'tr',
                  label: l10n.turkishLanguage,
                ),
                OpenChatSelectOption<String>(
                  value: 'en',
                  label: l10n.englishLanguage,
                ),
                OpenChatSelectOption<String>(
                  value: 'es',
                  label: l10n.spanishLanguage,
                ),
                OpenChatSelectOption<String>(
                  value: 'de',
                  label: l10n.germanLanguage,
                ),
                OpenChatSelectOption<String>(
                  value: 'fr',
                  label: l10n.frenchLanguage,
                ),
              ],
              value: languageCode,
              onChanged: _isSavingLanguage || widget.onLocaleChanged == null
                  ? null
                  : (value) {
                      switch (value) {
                        case 'system':
                          unawaited(_changeLocale(null));
                        case 'tr':
                          unawaited(_changeLocale(const Locale('tr')));
                        case 'en':
                          unawaited(_changeLocale(const Locale('en')));
                        case 'es':
                          unawaited(_changeLocale(const Locale('es')));
                        case 'de':
                          unawaited(_changeLocale(const Locale('de')));
                        case 'fr':
                          unawaited(_changeLocale(const Locale('fr')));
                        default:
                          throw StateError(
                            'Unsupported language preference: $value',
                          );
                      }
                    },
              palette: palette,
              width: width,
            );
          },
        ),
        const SizedBox(height: 18),
        _SettingsRow(
          title: l10n.conversationWidth,
          description: l10n.conversationWidthDescription,
          textTheme: textTheme,
          controlWidth: 264,
          desktopControlTopInset: 0,
          controlBuilder: (width) =>
              _SegmentedSelector<ConversationWidthPreference>(
                width: width,
                value: widget.conversationWidth,
                options: [
                  (ConversationWidthPreference.narrow, l10n.widthNarrow),
                  (ConversationWidthPreference.normal, l10n.widthNormal),
                  (ConversationWidthPreference.wide, l10n.widthWide),
                ],
                onChanged:
                    widget.onConversationWidthChanged == null ||
                        _isSavingAppearancePreference
                    ? null
                    : (value) => unawaited(_changeConversationWidth(value)),
                palette: palette,
              ),
        ),
        const SizedBox(height: 18),
        _SettingsRow(
          title: l10n.conversationTextSize,
          description: l10n.conversationTextSizeDescription,
          textTheme: textTheme,
          controlWidth: 264,
          desktopControlTopInset: 0,
          controlBuilder: (width) =>
              _SegmentedSelector<ConversationTextSizePreference>(
                width: width,
                value: widget.conversationTextSize,
                options: [
                  (ConversationTextSizePreference.small, l10n.textSizeSmall),
                  (ConversationTextSizePreference.normal, l10n.textSizeNormal),
                  (ConversationTextSizePreference.large, l10n.textSizeLarge),
                ],
                onChanged:
                    widget.onConversationTextSizeChanged == null ||
                        _isSavingAppearancePreference
                    ? null
                    : (value) => unawaited(_changeConversationTextSize(value)),
                palette: palette,
              ),
        ),
        const SizedBox(height: 18),
        _SettingsRow(
          title: l10n.appFont,
          description: l10n.appFontDescription,
          textTheme: textTheme,
          controlWidth: 264,
          desktopControlTopInset: 0,
          controlBuilder: (width) => OpenChatSelect<AppFontPreference>(
            options: [
              for (final font in AppFontPreference.values)
                OpenChatSelectOption<AppFontPreference>(
                  value: font,
                  label: font.familyName,
                  textStyle: TextStyle(fontFamily: font.familyName),
                ),
            ],
            value: widget.appFont,
            onChanged:
                widget.onAppFontChanged == null || _isSavingAppearancePreference
                ? null
                : (value) => unawaited(_changeAppFont(value)),
            palette: palette,
            width: width,
            height: 40,
          ),
        ),
      ],
    );
  }

  Widget _buildLocalDataSection() {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final textTheme = Theme.of(context).textTheme;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _SettingsRow(
          title: l10n.conversationHistory,
          description: switch (widget.historyStorageStatus) {
            HistoryStorageStatus.loading => l10n.historyCheckingDescription,
            HistoryStorageStatus.available => l10n.historyDeviceDescription,
            HistoryStorageStatus.unavailable =>
              l10n.historyStorageUnavailableDescription,
            HistoryStorageStatus.corrupt =>
              l10n.historyStorageCorruptDescription,
            HistoryStorageStatus.backupUnavailable =>
              l10n.historyStorageBackupFailedDescription,
          },
          textTheme: textTheme,
          controlWidth: 144,
          desktopControlTopInset: 4,
          controlBuilder: (width) => _StatusLabel(
            width: width,
            label: switch (widget.historyStorageStatus) {
              HistoryStorageStatus.loading => l10n.historyCheckingStatus,
              HistoryStorageStatus.available => l10n.historyDeviceStatus,
              HistoryStorageStatus.unavailable =>
                l10n.historyStorageUnavailableStatus,
              HistoryStorageStatus.corrupt ||
              HistoryStorageStatus.backupUnavailable =>
                l10n.historyStorageUnavailableStatus,
            },
            palette: palette,
          ),
        ),
        const SizedBox(height: 22),
        ConversationArchiveSection(
          serviceClient: widget.serviceClient,
          chatRepository: widget.chatRepository,
        ),
        const SizedBox(height: 16),
        ProfileArchiveSection(serviceClient: widget.serviceClient),
        const SizedBox(height: 22),
        _SettingsRow(
          title: l10n.clearConversationHistory,
          description: l10n.clearConversationHistoryDescription,
          textTheme: textTheme,
          controlWidth: 176,
          desktopControlTopInset: 0,
          controlBuilder: (width) => SizedBox(
            width: width,
            height: 36,
            child: OutlinedButton.icon(
              onPressed:
                  widget.hasConversationHistory &&
                      widget.onClearConversationHistory != null &&
                      !_isClearingHistory
                  ? () => unawaited(_confirmClearHistory())
                  : null,
              style:
                  OutlinedButton.styleFrom(
                    foregroundColor: Theme.of(context).colorScheme.error,
                    disabledForegroundColor: palette.disabledForeground,
                    disabledBackgroundColor: palette.disabledSurface,
                    side: BorderSide(
                      color: Theme.of(context).colorScheme.error,
                    ),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(8),
                    ),
                  ).copyWith(
                    side: WidgetStateProperty.resolveWith<BorderSide?>((
                      states,
                    ) {
                      final color = states.contains(WidgetState.disabled)
                          ? palette.disabledBorder
                          : Theme.of(context).colorScheme.error;
                      return BorderSide(color: color);
                    }),
                  ),
              icon: _isClearingHistory
                  ? const SizedBox.square(
                      dimension: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.trash2, size: 18),
              label: Text(
                _isClearingHistory ? l10n.clearingHistory : l10n.deleteAll,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ),
        ),
      ],
    );
  }

  Future<void> _loadSharedInstructions() async {
    setState(() {
      _isLoadingInstructions = true;
      _sharedInstructionsError = null;
    });
    try {
      final instructions = await widget.settingsPreferences
          .readSharedInstructions();
      if (!mounted) return;
      _sharedInstructionsController.text = instructions;
      setState(() {
        _savedSharedInstructions = instructions;
        _isLoadingInstructions = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading shared chat instructions'),
        ),
      );
      if (!mounted) return;
      setState(() {
        _sharedInstructionsError =
            context.openchatL10n.sharedInstructionsLoadFailed;
        _isLoadingInstructions = false;
      });
    }
  }

  Future<void> _saveSharedInstructions() async {
    if (_isSavingInstructions || _isLoadingInstructions) return;
    setState(() => _isSavingInstructions = true);
    try {
      final instructions = _sharedInstructionsController.text;
      await widget.settingsPreferences.writeSharedInstructions(instructions);
      if (!mounted) return;
      setState(() => _savedSharedInstructions = instructions);
      showOpenChatToast(
        context,
        context.openchatL10n.sharedInstructionsSaved,
        type: OpenChatToastType.success,
      );
    } on ArgumentError {
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.sharedInstructionsTooLong,
        type: OpenChatToastType.error,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while saving shared chat instructions'),
        ),
      );
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.sharedInstructionsSaveFailed,
        type: OpenChatToastType.error,
      );
    } finally {
      if (mounted) setState(() => _isSavingInstructions = false);
    }
  }

  Future<void> _changeThemeMode(ThemeMode mode) async {
    try {
      await widget.onThemeModeChanged(mode);
    } on PlatformException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while saving the theme preference'),
        ),
      );
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.themeSaveFailed,
        type: OpenChatToastType.error,
      );
    }
  }

  Future<void> _changeConversationWidth(
    ConversationWidthPreference width,
  ) async {
    final onChanged = widget.onConversationWidthChanged;
    if (onChanged == null) return;
    await _saveAppearancePreference(() => onChanged(width));
  }

  Future<void> _changeConversationTextSize(
    ConversationTextSizePreference size,
  ) async {
    final onChanged = widget.onConversationTextSizeChanged;
    if (onChanged == null) return;
    await _saveAppearancePreference(() => onChanged(size));
  }

  Future<void> _changeAppFont(AppFontPreference font) async {
    final onChanged = widget.onAppFontChanged;
    if (onChanged == null) return;
    await _saveAppearancePreference(() => onChanged(font));
  }

  Future<void> _saveAppearancePreference(Future<void> Function() save) async {
    if (_isSavingAppearancePreference) return;
    setState(() => _isSavingAppearancePreference = true);
    try {
      await save();
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while saving conversation appearance'),
        ),
      );
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.appearancePreferenceSaveFailed,
        type: OpenChatToastType.error,
      );
    } finally {
      if (mounted) setState(() => _isSavingAppearancePreference = false);
    }
  }

  Future<void> _changeLocale(Locale? locale) async {
    final changeLocale = widget.onLocaleChanged;
    if (_isSavingLanguage || changeLocale == null) return;
    setState(() => _isSavingLanguage = true);
    try {
      await changeLocale(locale);
    } on PlatformException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while saving the app language'),
        ),
      );
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.languageSaveFailed,
        type: OpenChatToastType.error,
      );
    } finally {
      if (mounted) {
        setState(() => _isSavingLanguage = false);
      }
    }
  }

  Future<void> _confirmClearHistory() async {
    final clearHistory = widget.onClearConversationHistory;
    if (clearHistory == null || !widget.hasConversationHistory) return;

    final l10n = context.openchatL10n;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.confirmClearHistoryTitle),
        content: Text(l10n.confirmClearHistoryBody),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(dialogContext).colorScheme.error,
            ),
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(l10n.deleteAll),
          ),
        ],
      ),
    );
    if (!mounted || confirmed != true) return;

    setState(() => _isClearingHistory = true);
    try {
      await clearHistory();
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.clearHistorySucceeded,
        type: OpenChatToastType.success,
      );
    } on Exception catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while clearing local chat history'),
        ),
      );
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.clearHistoryFailed,
        type: OpenChatToastType.error,
      );
    } finally {
      if (mounted) setState(() => _isClearingHistory = false);
    }
  }
}

typedef _SettingsControlBuilder = Widget Function(double width);

class _SettingsRow extends StatelessWidget {
  const _SettingsRow({
    required this.title,
    required this.description,
    required this.textTheme,
    required this.controlWidth,
    required this.desktopControlTopInset,
    required this.controlBuilder,
  });

  final String title;
  final String description;
  final TextTheme textTheme;
  final double controlWidth;
  final double desktopControlTopInset;
  final _SettingsControlBuilder controlBuilder;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < 560) {
          final width = constraints.maxWidth < controlWidth
              ? constraints.maxWidth
              : controlWidth;
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _SettingDescription(
                title: title,
                description: description,
                textTheme: textTheme,
              ),
              const SizedBox(height: 12),
              Align(
                alignment: Alignment.centerLeft,
                child: controlBuilder(width),
              ),
            ],
          );
        }

        return Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: _SettingDescription(
                title: title,
                description: description,
                textTheme: textTheme,
              ),
            ),
            const SizedBox(width: 24),
            Padding(
              padding: EdgeInsets.only(top: desktopControlTopInset),
              child: controlBuilder(controlWidth),
            ),
          ],
        );
      },
    );
  }
}

class _SettingsSidebar extends StatelessWidget {
  const _SettingsSidebar({
    required this.compact,
    required this.selectedSection,
    required this.advancedExpanded,
    required this.onToggleAdvanced,
    required this.onSelectSection,
  });

  final bool compact;
  final _SettingsSection selectedSection;
  final bool advancedExpanded;
  final VoidCallback onToggleAdvanced;
  final ValueChanged<_SettingsSection> onSelectSection;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final generalEntries = [
      (section: _SettingsSection.connections, label: l10n.connections),
      (
        section: _SettingsSection.sharedInstructions,
        label: l10n.sharedInstructions,
      ),
      (section: _SettingsSection.appearance, label: l10n.appearance),
    ];
    final recoveryEntry = (
      section: _SettingsSection.localData,
      label: l10n.localData,
    );
    final advancedEntries = [
      (section: _SettingsSection.usageQuotas, label: l10n.usageQuotas),
      (section: _SettingsSection.statistics, label: l10n.statistics),
      (section: _SettingsSection.models, label: l10n.modelPreferences),
      (section: _SettingsSection.localEngines, label: l10n.localEngines),
      (
        section: _SettingsSection.conversationMemory,
        label: l10n.conversationMemory,
      ),
    ];

    return Container(
      width: compact ? 64 : 252,
      decoration: BoxDecoration(color: palette.navigation),
      foregroundDecoration: BoxDecoration(
        border: Border(right: BorderSide(color: palette.border)),
      ),
      child: SafeArea(
        child: Padding(
          padding: EdgeInsets.fromLTRB(
            compact ? 8 : 18,
            18,
            compact ? 8 : 14,
            16,
          ),
          child: SingleChildScrollView(
            primary: false,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (compact)
                  SizedBox(
                    height: 38,
                    child: Center(
                      child: Icon(
                        LucideIcons.settings,
                        color: palette.secondaryIcon,
                        size: 20,
                      ),
                    ),
                  )
                else ...[
                  Text(
                    l10n.settings,
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                ],
                const SizedBox(height: 16),
                Divider(height: 1, color: palette.border),
                const SizedBox(height: 12),
                if (!compact) ...[
                  _SettingsGroupLabel(label: l10n.settingsGeneral),
                  const SizedBox(height: 6),
                ],
                for (final entry in generalEntries) ...[
                  _SettingsSidebarItem(
                    compact: compact,
                    label: entry.label,
                    icon: entry.section.icon,
                    selected: selectedSection == entry.section,
                    onPressed: () => onSelectSection(entry.section),
                  ),
                  const SizedBox(height: 4),
                ],
                if (!compact) ...[
                  const SizedBox(height: 8),
                  Divider(height: 1, color: palette.border),
                  const SizedBox(height: 10),
                  _SettingsGroupLabel(label: l10n.settingsDataRecovery),
                  const SizedBox(height: 6),
                ],
                _SettingsSidebarItem(
                  compact: compact,
                  label: recoveryEntry.label,
                  icon: recoveryEntry.section.icon,
                  selected: selectedSection == recoveryEntry.section,
                  onPressed: () => onSelectSection(recoveryEntry.section),
                ),
                const SizedBox(height: 8),
                if (!compact) Divider(height: 1, color: palette.border),
                if (!compact) const SizedBox(height: 10),
                Semantics(
                  button: true,
                  expanded: advancedExpanded,
                  label: l10n.settingsAdvanced,
                  onTap: onToggleAdvanced,
                  child: ExcludeSemantics(
                    child: Tooltip(
                      message: l10n.settingsAdvanced,
                      child: Material(
                        color: Colors.transparent,
                        borderRadius: BorderRadius.circular(8),
                        child: InkWell(
                          onTap: onToggleAdvanced,
                          borderRadius: BorderRadius.circular(8),
                          child: ConstrainedBox(
                            constraints: const BoxConstraints(minHeight: 44),
                            child: Padding(
                              padding: EdgeInsets.symmetric(
                                horizontal: compact ? 0 : 12,
                              ),
                              child: Row(
                                mainAxisAlignment: compact
                                    ? MainAxisAlignment.center
                                    : MainAxisAlignment.start,
                                children: [
                                  Icon(
                                    LucideIcons.slidersHorizontal,
                                    size: 18,
                                    color: advancedExpanded
                                        ? palette.text
                                        : palette.secondaryIcon,
                                  ),
                                  if (!compact) ...[
                                    const SizedBox(width: 12),
                                    Expanded(
                                      child: Text(
                                        l10n.settingsAdvanced,
                                        style: TextStyle(
                                          color: palette.secondaryText,
                                          fontSize: 13,
                                        ),
                                      ),
                                    ),
                                    Icon(
                                      advancedExpanded
                                          ? LucideIcons.chevronDown
                                          : LucideIcons.chevronRight,
                                      size: 16,
                                      color: palette.secondaryIcon,
                                    ),
                                  ],
                                ],
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
                if (advancedExpanded) ...[
                  const SizedBox(height: 6),
                  for (final entry in advancedEntries) ...[
                    _SettingsSidebarItem(
                      compact: compact,
                      label: entry.label,
                      icon: entry.section.icon,
                      selected: selectedSection == entry.section,
                      onPressed: () => onSelectSection(entry.section),
                    ),
                    const SizedBox(height: 4),
                  ],
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _SettingsGroupLabel extends StatelessWidget {
  const _SettingsGroupLabel({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      child: Text(
        label,
        style: TextStyle(
          color: palette.secondaryText,
          fontSize: OpenChatTypography.metadata,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}

class _SettingsSidebarItem extends StatefulWidget {
  const _SettingsSidebarItem({
    required this.compact,
    required this.label,
    required this.icon,
    required this.selected,
    required this.onPressed,
  });

  final bool compact;
  final String label;
  final IconData icon;
  final bool selected;
  final VoidCallback onPressed;

  @override
  State<_SettingsSidebarItem> createState() => _SettingsSidebarItemState();
}

class _SettingsSidebarItemState extends State<_SettingsSidebarItem> {
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);

    return Semantics(
      button: true,
      enabled: true,
      selected: widget.selected,
      label: widget.label,
      onTap: widget.onPressed,
      child: ExcludeSemantics(
        child: Tooltip(
          message: widget.label,
          child: Material(
            color: widget.selected ? palette.selected : Colors.transparent,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(8),
              side: _focused
                  ? BorderSide(color: palette.focusRing, width: 2)
                  : BorderSide.none,
            ),
            clipBehavior: Clip.antiAlias,
            child: InkWell(
              onTap: widget.onPressed,
              onFocusChange: (focused) {
                if (_focused != focused) setState(() => _focused = focused);
              },
              borderRadius: BorderRadius.circular(8),
              overlayColor: WidgetStateProperty.resolveWith((states) {
                if (states.contains(WidgetState.pressed)) {
                  return palette.selected;
                }
                if (states.contains(WidgetState.hovered)) return palette.hover;
                return null;
              }),
              child: ConstrainedBox(
                constraints: const BoxConstraints(minHeight: 44),
                child: Padding(
                  padding: EdgeInsets.symmetric(
                    horizontal: widget.compact ? 0 : 12,
                  ),
                  child: Row(
                    mainAxisAlignment: widget.compact
                        ? MainAxisAlignment.center
                        : MainAxisAlignment.start,
                    children: [
                      Icon(
                        widget.icon,
                        color: widget.selected
                            ? palette.text
                            : palette.secondaryIcon,
                        size: 18,
                      ),
                      if (!widget.compact) ...[
                        const SizedBox(width: 12),
                        Expanded(
                          child: Text(
                            widget.label,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: widget.selected
                                  ? palette.text
                                  : palette.secondaryText,
                              fontSize: 13,
                              fontWeight: widget.selected
                                  ? FontWeight.w600
                                  : FontWeight.w400,
                            ),
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _SettingDescription extends StatelessWidget {
  const _SettingDescription({
    required this.title,
    required this.description,
    required this.textTheme,
  });

  final String title;
  final String description;
  final TextTheme textTheme;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        ConstrainedBox(
          constraints: const BoxConstraints(minHeight: 22),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(
              title,
              style: TextStyle(
                color: palette.text,
                fontSize: 15,
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
        ),
        const SizedBox(height: 2),
        Text(
          description,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 13,
            fontWeight: FontWeight.w400,
          ),
        ),
      ],
    );
  }
}

class _SegmentedSelector<T> extends StatelessWidget {
  const _SegmentedSelector({
    required this.width,
    required this.value,
    required this.options,
    required this.onChanged,
    required this.palette,
  });

  final double width;
  final T value;
  final List<(T, String)> options;
  final ValueChanged<T>? onChanged;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final changeSelection = onChanged;
    final selectorOptions = <Widget>[];
    for (final option in options) {
      if (selectorOptions.isNotEmpty) {
        selectorOptions.add(const SizedBox(width: 4));
      }
      selectorOptions.add(
        Expanded(
          child: _ThemeChoice(
            label: option.$2,
            selected: option.$1 == value,
            selectedSemanticsLabel: option.$2,
            palette: palette,
            onPressed: changeSelection == null
                ? null
                : () => changeSelection(option.$1),
          ),
        ),
      );
    }

    return Container(
      width: width,
      constraints: const BoxConstraints(minHeight: 40),
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: changeSelection == null
            ? palette.disabledSurface
            : palette.hover,
        border: Border.all(
          color: changeSelection == null
              ? palette.disabledBorder
              : palette.controlBorder,
        ),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(children: selectorOptions),
    );
  }
}

class _ThemeChoice extends StatelessWidget {
  const _ThemeChoice({
    required this.label,
    required this.selected,
    required this.selectedSemanticsLabel,
    required this.palette,
    required this.onPressed,
  });

  final String label;
  final bool selected;
  final String selectedSemanticsLabel;
  final OpenChatPalette palette;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final isEnabled = onPressed != null;

    return Semantics(
      button: true,
      enabled: onPressed != null,
      selected: selected,
      label: selectedSemanticsLabel,
      onTap: onPressed,
      child: ExcludeSemantics(
        child: Material(
          color: selected ? palette.selected : Colors.transparent,
          borderRadius: BorderRadius.circular(6),
          child: InkWell(
            onTap: onPressed,
            borderRadius: BorderRadius.circular(6),
            child: Center(
              child: Text(
                label,
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                  color: selected
                      ? palette.text
                      : isEnabled
                      ? palette.secondaryText
                      : palette.disabledForeground,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _StatusLabel extends StatelessWidget {
  const _StatusLabel({
    required this.width,
    required this.label,
    required this.palette,
  });

  final double width;
  final String label;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: width,
      height: 30,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: palette.hover,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Text(
        label,
        style: TextStyle(
          color: palette.accent,
          fontSize: 12,
          fontWeight: FontWeight.w500,
        ),
      ),
    );
  }
}
