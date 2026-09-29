import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../app/openchat_theme.dart';
import '../../../app/openchat_toast.dart';
import '../../../l10n/openchat_localizations.dart';
import '../../../platform/windows/openchat_service_client.dart';
import '../../../platform/windows/window_controls.dart';
import '../../chat/domain/history_storage_status.dart';
import '../../chat/presentation/widgets/window_control_bar.dart';
import '../data/chat_gpt_api_key_store.dart';
import '../data/open_code_api_key_store.dart';
import '../data/settings_preferences.dart';
import 'chat_gpt_connection_section.dart';
import 'open_code_connection_section.dart';
import 'settings_widgets.dart';

const _sharedInstructionsMaxLength = 4096;

enum _SettingsSection { connections, sharedInstructions, appearance, localData }

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    required this.themeMode,
    required this.onThemeModeChanged,
    required this.historyStorageStatus,
    required this.hasConversationHistory,
    required this.settingsPreferences,
    this.locale,
    this.onLocaleChanged,
    this.onClearConversationHistory,
    this.chatGptApiKeyStore,
    this.openCodeApiKeyStore,
    this.serviceClient,
    this.onProviderStateChanged,
    this.onConnectionRemoved,
    super.key,
  });

  final ThemeMode themeMode;
  final Locale? locale;
  final Future<void> Function(ThemeMode) onThemeModeChanged;
  final Future<void> Function(Locale?)? onLocaleChanged;
  final HistoryStorageStatus historyStorageStatus;
  final bool hasConversationHistory;
  final SettingsPreferences settingsPreferences;
  final Future<void> Function()? onClearConversationHistory;
  final ChatGptApiKeyStore? chatGptApiKeyStore;
  final OpenCodeApiKeyStore? openCodeApiKeyStore;
  final OpenChatServiceClient? serviceClient;
  final Future<void> Function()? onProviderStateChanged;
  final Future<void> Function(String connectionId)? onConnectionRemoved;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  late final TextEditingController _sharedInstructionsController;
  bool _isClearingHistory = false;
  bool _isSavingLanguage = false;
  bool _isLoadingInstructions = true;
  bool _isSavingInstructions = false;
  String _savedSharedInstructions = '';
  String? _sharedInstructionsError;
  int _languageSelectorRevision = 0;
  _SettingsSection _selectedSection = _SettingsSection.connections;

  @override
  void initState() {
    super.initState();
    _sharedInstructionsController = TextEditingController();
    unawaited(_loadSharedInstructions());
  }

  @override
  void dispose() {
    _sharedInstructionsController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;

    return LayoutBuilder(
      builder: (context, constraints) {
        final compactSidebar = constraints.maxWidth < 960;
        final horizontalInset = constraints.maxWidth < 640 ? 16.0 : 24.0;
        final windowControlInset = constraints.maxWidth < 640 ? 16.0 : 32.0;

        return ColoredBox(
          color: palette.surface,
          child: Row(
            children: [
              _SettingsSidebar(
                compact: compactSidebar,
                selectedSection: _selectedSection,
                onSelectSection: (section) =>
                    setState(() => _selectedSection = section),
              ),
              Expanded(
                child: Stack(
                  children: [
                    IndexedStack(
                      index: _selectedSection.index,
                      children: [
                        _buildSectionPage(
                          section: _SettingsSection.connections,
                          title: l10n.connections,
                          horizontalInset: horizontalInset,
                          child: _buildConnectionsSection(),
                        ),
                        _buildSectionPage(
                          section: _SettingsSection.sharedInstructions,
                          title: l10n.sharedInstructions,
                          horizontalInset: horizontalInset,
                          child: _buildSharedInstructionsSection(),
                        ),
                        _buildSectionPage(
                          section: _SettingsSection.appearance,
                          title: l10n.appearance,
                          horizontalInset: horizontalInset,
                          child: _buildAppearanceSection(),
                        ),
                        _buildSectionPage(
                          section: _SettingsSection.localData,
                          title: l10n.localData,
                          horizontalInset: horizontalInset,
                          child: _buildLocalDataSection(),
                        ),
                      ],
                    ),
                    if (OpenChatWindowControls.isSupported)
                      Positioned(
                        right: windowControlInset,
                        top: 18,
                        child: const WindowControlBar(),
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
    required String title,
    required double horizontalInset,
    required Widget child,
  }) {
    final palette = OpenChatPalette.of(context);
    final textTheme = Theme.of(context).textTheme;

    return SingleChildScrollView(
      key: PageStorageKey<_SettingsSection>(section),
      padding: EdgeInsets.fromLTRB(horizontalInset, 52, horizontalInset, 32),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 840),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SizedBox(
                height: 40,
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    title,
                    style: textTheme.headlineSmall?.copyWith(
                      fontSize: 30,
                      height: 4 / 3,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 20),
              SettingsDivider(color: palette.border),
              const SizedBox(height: 33),
              child,
            ],
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
                icon: const Icon(Icons.refresh_rounded),
                label: Text(l10n.retry),
              ),
            ],
          )
        else ...[
          TextField(
            controller: _sharedInstructionsController,
            enabled: !_isSavingInstructions,
            minLines: 3,
            maxLines: 8,
            maxLength: _sharedInstructionsMaxLength,
            buildCounter: (
              context, {
              required currentLength,
              required isFocused,
              maxLength,
            }) => null,
            textCapitalization: TextCapitalization.sentences,
            decoration: InputDecoration(
              hintText: l10n.sharedInstructionsHint,
              alignLabelWithHint: true,
              border: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
              ),
            ),
            onChanged: (_) => setState(() {}),
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              Text(
                '${_sharedInstructionsController.text.characters.length}/$_sharedInstructionsMaxLength',
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
              const Spacer(),
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
                    : const Icon(Icons.save_outlined),
                label: Text(_isSavingInstructions ? l10n.saving : l10n.save),
              ),
            ],
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
          controlBuilder: (width) => _ThemeSelector(
            width: width,
            mode: widget.themeMode,
            onChanged: (mode) => unawaited(_changeThemeMode(mode)),
            systemLabel: l10n.systemTheme,
            lightLabel: l10n.lightTheme,
            darkLabel: l10n.darkTheme,
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
            return SizedBox(
              width: width,
              height: 40,
              child: DropdownButtonFormField<String>(
                key: ValueKey<String>(
                  '$languageCode-$_languageSelectorRevision',
                ),
                initialValue: languageCode,
                isExpanded: true,
                decoration: InputDecoration(
                  contentPadding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 10,
                  ),
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(8),
                  ),
                ),
                items: [
                  DropdownMenuItem<String>(
                    value: 'system',
                    child: Text(l10n.systemLanguage),
                  ),
                  DropdownMenuItem<String>(
                    value: 'tr',
                    child: Text(l10n.turkishLanguage),
                  ),
                  DropdownMenuItem<String>(
                    value: 'en',
                    child: Text(l10n.englishLanguage),
                  ),
                ],
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
                          case null:
                            break;
                          default:
                            throw StateError(
                              'Unsupported language preference: $value',
                            );
                        }
                      },
              ),
            );
          },
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
            },
            palette: palette,
          ),
        ),
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
              style: OutlinedButton.styleFrom(
                foregroundColor: Theme.of(context).colorScheme.error,
                disabledForegroundColor: palette.secondaryText,
                side: BorderSide(color: Theme.of(context).colorScheme.error),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(8),
                ),
              ),
              icon: _isClearingHistory
                  ? const SizedBox.square(
                      dimension: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.delete_outline_rounded, size: 18),
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
        setState(() {
          _isSavingLanguage = false;
          _languageSelectorRevision++;
        });
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
    required this.onSelectSection,
  });

  final bool compact;
  final _SettingsSection selectedSection;
  final ValueChanged<_SettingsSection> onSelectSection;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final entries = [
      (
        section: _SettingsSection.connections,
        label: l10n.connections,
        icon: Icons.link_rounded,
      ),
      (
        section: _SettingsSection.sharedInstructions,
        label: l10n.sharedInstructions,
        icon: Icons.notes_rounded,
      ),
      (
        section: _SettingsSection.appearance,
        label: l10n.appearance,
        icon: Icons.palette_outlined,
      ),
      (
        section: _SettingsSection.localData,
        label: l10n.localData,
        icon: Icons.storage_rounded,
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
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (compact)
                SizedBox(
                  height: 38,
                  child: Center(
                    child: Icon(
                      Icons.settings_outlined,
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
                const SizedBox(height: 6),
                Text(
                  l10n.settingsDescription,
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: 12,
                    height: 1.4,
                  ),
                ),
              ],
              const SizedBox(height: 16),
              Divider(height: 1, color: palette.border),
              const SizedBox(height: 12),
              for (final entry in entries) ...[
                _SettingsSidebarItem(
                  compact: compact,
                  label: entry.label,
                  icon: entry.icon,
                  selected: selectedSection == entry.section,
                  onPressed: () => onSelectSection(entry.section),
                ),
                const SizedBox(height: 4),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _SettingsSidebarItem extends StatelessWidget {
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
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);

    return Semantics(
      button: true,
      selected: selected,
      label: label,
      child: ExcludeSemantics(
        child: Tooltip(
          message: label,
          child: Material(
            color: selected ? palette.selected : Colors.transparent,
            borderRadius: BorderRadius.circular(8),
            child: InkWell(
              onTap: onPressed,
              borderRadius: BorderRadius.circular(8),
              overlayColor: WidgetStateProperty.resolveWith((states) {
                if (states.contains(WidgetState.hovered)) return palette.hover;
                if (states.contains(WidgetState.pressed)) {
                  return palette.selected;
                }
                return null;
              }),
              child: SizedBox(
                height: 44,
                child: Padding(
                  padding: EdgeInsets.symmetric(horizontal: compact ? 0 : 12),
                  child: Row(
                    mainAxisAlignment: compact
                        ? MainAxisAlignment.center
                        : MainAxisAlignment.start,
                    children: [
                      Icon(
                        icon,
                        color: selected
                            ? palette.accentIcon
                            : palette.secondaryIcon,
                        size: 18,
                      ),
                      if (!compact) ...[
                        const SizedBox(width: 12),
                        Expanded(
                          child: Text(
                            label,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: selected
                                  ? palette.text
                                  : palette.secondaryText,
                              fontSize: 13,
                              fontWeight: selected
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
        SizedBox(
          height: 22,
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

class _ThemeSelector extends StatelessWidget {
  const _ThemeSelector({
    required this.width,
    required this.mode,
    required this.onChanged,
    required this.systemLabel,
    required this.lightLabel,
    required this.darkLabel,
    required this.palette,
  });

  final double width;
  final ThemeMode mode;
  final ValueChanged<ThemeMode> onChanged;
  final String systemLabel;
  final String lightLabel;
  final String darkLabel;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: width,
      height: 40,
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: palette.hover,
        border: Border.all(color: palette.controlBorder),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        children: [
          Expanded(
            child: _ThemeChoice(
              label: systemLabel,
              selected: mode == ThemeMode.system,
              selectedSemanticsLabel: systemLabel,
              palette: palette,
              onPressed: () => onChanged(ThemeMode.system),
            ),
          ),
          const SizedBox(width: 4),
          Expanded(
            child: _ThemeChoice(
              label: lightLabel,
              selected: mode == ThemeMode.light,
              selectedSemanticsLabel: lightLabel,
              palette: palette,
              onPressed: () => onChanged(ThemeMode.light),
            ),
          ),
          const SizedBox(width: 4),
          Expanded(
            child: _ThemeChoice(
              label: darkLabel,
              selected: mode == ThemeMode.dark,
              selectedSemanticsLabel: darkLabel,
              palette: palette,
              onPressed: () => onChanged(ThemeMode.dark),
            ),
          ),
        ],
      ),
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
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      button: true,
      selected: selected,
      label: selectedSemanticsLabel,
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
                  color: selected ? palette.text : palette.secondaryText,
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
