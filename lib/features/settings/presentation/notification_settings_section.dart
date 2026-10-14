import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/windows_notification_status.dart';

class NotificationSettingsSection extends StatefulWidget {
  const NotificationSettingsSection({
    required this.settingsPreferences,
    this.notificationStatusClient = const WindowsNotificationStatusClient(),
    this.onSendTestNotification,
    super.key,
  });

  final SettingsPreferences settingsPreferences;
  final WindowsNotificationStatusClient notificationStatusClient;
  final Future<bool> Function()? onSendTestNotification;

  @override
  State<NotificationSettingsSection> createState() =>
      _NotificationSettingsSectionState();
}

class _NotificationSettingsSectionState
    extends State<NotificationSettingsSection>
    with WidgetsBindingObserver {
  ConversationNotificationPreferences? _preferences;
  bool _isLoading = true;
  bool _isSaving = false;
  bool _isSendingTestNotification = false;
  bool _loadFailed = false;
  WindowsNotificationStatus? _deliveryStatus;
  bool _isLoadingDeliveryStatus = true;
  bool _deliveryStatusFailed = false;
  int _deliveryStatusGeneration = 0;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    unawaited(_loadPreferences());
    unawaited(_loadDeliveryStatus());
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(_loadDeliveryStatus());
    }
  }

  Future<void> _loadDeliveryStatus() async {
    final generation = ++_deliveryStatusGeneration;
    setState(() {
      _isLoadingDeliveryStatus = true;
      _deliveryStatusFailed = false;
    });
    try {
      final status = await widget.notificationStatusClient.readStatus();
      if (!mounted || generation != _deliveryStatusGeneration) return;
      setState(() {
        _deliveryStatus = status;
        _isLoadingDeliveryStatus = false;
        _deliveryStatusFailed = status == WindowsNotificationStatus.unavailable;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'notifications',
          context: ErrorDescription(
            'while checking Windows notification status',
          ),
        ),
      );
      if (!mounted || generation != _deliveryStatusGeneration) return;
      setState(() {
        _deliveryStatus = WindowsNotificationStatus.unavailable;
        _isLoadingDeliveryStatus = false;
        _deliveryStatusFailed = true;
      });
    }
  }

  Future<void> _openNotificationSettings() async {
    try {
      await widget.notificationStatusClient.openSettings();
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'notifications',
          context: ErrorDescription(
            'while opening Windows notification settings',
          ),
        ),
      );
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.notificationSettingsUnavailable,
        type: OpenChatToastType.error,
      );
    }
  }

  Future<void> _sendTestNotification() async {
    final sendTestNotification = widget.onSendTestNotification;
    if (sendTestNotification == null || _isSendingTestNotification) return;
    setState(() => _isSendingTestNotification = true);
    try {
      final requested = await sendTestNotification();
      if (!mounted) return;
      showOpenChatToast(
        context,
        requested
            ? context.openchatL10n.notificationTestRequested
            : context.openchatL10n.notificationSettingsUnavailable,
        type: requested ? OpenChatToastType.info : OpenChatToastType.error,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'notifications',
          context: ErrorDescription('while sending a test notification'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.notificationTestFailed,
          type: OpenChatToastType.error,
        );
      }
    } finally {
      if (mounted) setState(() => _isSendingTestNotification = false);
    }
  }

  String _deliveryStatusLabel(BuildContext context) {
    final l10n = context.openchatL10n;
    return switch (_deliveryStatus) {
      WindowsNotificationStatus.enabled => l10n.notificationDeliveryEnabled,
      WindowsNotificationStatus.disabledForApplication =>
        l10n.notificationDeliveryDisabledForApp,
      WindowsNotificationStatus.disabledForUser =>
        l10n.notificationDeliveryDisabledForUser,
      WindowsNotificationStatus.disabledByGroupPolicy =>
        l10n.notificationDeliveryDisabledByPolicy,
      WindowsNotificationStatus.disabledByManifest =>
        l10n.notificationDeliveryDisabledByManifest,
      WindowsNotificationStatus.notSupported =>
        l10n.notificationDeliveryNotSupported,
      WindowsNotificationStatus.unavailable ||
      null => l10n.notificationDeliveryUnavailable,
    };
  }

  Future<void> _loadPreferences() async {
    setState(() {
      _isLoading = true;
      _loadFailed = false;
    });
    try {
      final preferences = await widget.settingsPreferences
          .readConversationNotificationPreferences();
      if (!mounted) return;
      setState(() {
        _preferences = preferences;
        _isLoading = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading notification preferences'),
        ),
      );
      if (!mounted) return;
      setState(() {
        _isLoading = false;
        _loadFailed = true;
      });
    }
  }

  Future<void> _save(
    ConversationNotificationPreferences updated,
    Future<void> Function() write,
  ) async {
    if (_isSaving) return;
    setState(() => _isSaving = true);
    try {
      await write();
      if (mounted) setState(() => _preferences = updated);
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while saving notification preferences'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.notificationSettingsSaveFailed,
          type: OpenChatToastType.error,
        );
      }
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final preferences = _preferences;

    if (_isLoading) {
      return const Center(
        child: SizedBox.square(
          dimension: 22,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      );
    }
    if (_loadFailed || preferences == null) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l10n.notificationSettingsUnavailable,
            style: TextStyle(color: palette.secondaryText),
          ),
          const SizedBox(height: 12),
          OutlinedButton.icon(
            onPressed: () => unawaited(_loadPreferences()),
            icon: const Icon(LucideIcons.refreshCw, size: 16),
            label: Text(l10n.retry),
          ),
        ],
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _buildDeliveryStatus(context, palette),
        const SizedBox(height: 12),
        SwitchListTile.adaptive(
          contentPadding: EdgeInsets.zero,
          title: Text(l10n.notificationResponses),
          subtitle: Text(l10n.notificationResponsesDescription),
          value: preferences.responses,
          onChanged: _isSaving
              ? null
              : (value) => unawaited(
                  _save(
                    preferences.copyWith(responses: value),
                    () => widget.settingsPreferences.writeNotificationResponses(
                      value,
                    ),
                  ),
                ),
        ),
        SwitchListTile.adaptive(
          contentPadding: EdgeInsets.zero,
          title: Text(l10n.notificationQuestions),
          subtitle: Text(l10n.notificationQuestionsDescription),
          value: preferences.questions,
          onChanged: _isSaving
              ? null
              : (value) => unawaited(
                  _save(
                    preferences.copyWith(questions: value),
                    () => widget.settingsPreferences.writeNotificationQuestions(
                      value,
                    ),
                  ),
                ),
        ),
        SwitchListTile.adaptive(
          contentPadding: EdgeInsets.zero,
          title: Text(l10n.notificationResponsePreview),
          subtitle: Text(l10n.notificationResponsePreviewDescription),
          value: preferences.responsePreview,
          onChanged: _isSaving || !preferences.responses
              ? null
              : (value) => unawaited(
                  _save(
                    preferences.copyWith(responsePreview: value),
                    () => widget.settingsPreferences
                        .writeNotificationResponsePreview(value),
                  ),
                ),
        ),
        SwitchListTile.adaptive(
          contentPadding: EdgeInsets.zero,
          title: Text(l10n.notificationQuickReply),
          subtitle: Text(l10n.notificationQuickReplyDescription),
          value: preferences.quickReply,
          onChanged: _isSaving || !preferences.responses
              ? null
              : (value) => unawaited(
                  _save(
                    preferences.copyWith(quickReply: value),
                    () => widget.settingsPreferences
                        .writeNotificationQuickReply(value),
                  ),
                ),
        ),
      ],
    );
  }

  Widget _buildDeliveryStatus(BuildContext context, OpenChatPalette palette) {
    final l10n = context.openchatL10n;
    final status = _deliveryStatus;
    final isBlocked =
        status != null &&
        status != WindowsNotificationStatus.enabled &&
        status != WindowsNotificationStatus.notSupported &&
        status != WindowsNotificationStatus.unavailable;
    return Semantics(
      liveRegion: true,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: palette.composer,
          borderRadius: BorderRadius.circular(10),
        ),
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(
                status == WindowsNotificationStatus.enabled
                    ? LucideIcons.bellRing
                    : LucideIcons.bellOff,
                size: 18,
                color: status == WindowsNotificationStatus.enabled
                    ? palette.success
                    : palette.secondaryIcon,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      _isLoadingDeliveryStatus
                          ? l10n.notificationDeliveryChecking
                          : _deliveryStatusLabel(context),
                    ),
                    if (isBlocked || _deliveryStatusFailed) ...[
                      const SizedBox(height: 8),
                      Wrap(
                        spacing: 8,
                        runSpacing: 4,
                        children: [
                          TextButton(
                            onPressed: _isLoadingDeliveryStatus
                                ? null
                                : () => unawaited(_openNotificationSettings()),
                            child: Text(l10n.openNotificationSettings),
                          ),
                          if (_deliveryStatusFailed)
                            TextButton(
                              onPressed: _isLoadingDeliveryStatus
                                  ? null
                                  : () => unawaited(_loadDeliveryStatus()),
                              child: Text(l10n.retry),
                            ),
                        ],
                      ),
                    ],
                    if (status == WindowsNotificationStatus.enabled &&
                        widget.onSendTestNotification != null) ...[
                      const SizedBox(height: 8),
                      TextButton.icon(
                        onPressed: _isSendingTestNotification
                            ? null
                            : () => unawaited(_sendTestNotification()),
                        icon: _isSendingTestNotification
                            ? const SizedBox.square(
                                dimension: 14,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(LucideIcons.send, size: 15),
                        label: Text(l10n.sendTestNotification),
                      ),
                    ],
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
