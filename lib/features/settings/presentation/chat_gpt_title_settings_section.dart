import 'dart:async';

import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/data/available_model_catalog_repository.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/domain/default_model_preference.dart';
import 'package:openchat/features/chat/domain/title_generation_preference.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/features/settings/data/chat_gpt_api_key_store.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/presentation/chat_gpt_title_preference_section.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

enum _TitleSettingsLoadState { loading, loaded, failed }

class ChatGptTitleSettingsSection extends StatefulWidget {
  const ChatGptTitleSettingsSection({
    required this.serviceClient,
    required this.settingsPreferences,
    required this.chatGptApiKeyStore,
    required this.apiCompatibleProviderKeyStore,
    super.key,
  });

  final OpenChatServiceClient? serviceClient;
  final SettingsPreferences settingsPreferences;
  final ChatGptApiKeyStore? chatGptApiKeyStore;
  final ApiCompatibleProviderKeyStore? apiCompatibleProviderKeyStore;

  @override
  State<ChatGptTitleSettingsSection> createState() =>
      _ChatGptTitleSettingsSectionState();
}

class _ChatGptTitleSettingsSectionState
    extends State<ChatGptTitleSettingsSection> {
  List<ChatGptModel> _models = const [];
  TitleGenerationPreference _preference = const TitleGenerationPreference();
  _TitleSettingsLoadState _loadState = _TitleSettingsLoadState.loading;
  bool _isSaving = false;
  int _requestGeneration = 0;

  @override
  void initState() {
    super.initState();
    unawaited(reload());
  }

  @override
  void didUpdateWidget(covariant ChatGptTitleSettingsSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.serviceClient != widget.serviceClient ||
        oldWidget.chatGptApiKeyStore != widget.chatGptApiKeyStore ||
        oldWidget.apiCompatibleProviderKeyStore !=
            widget.apiCompatibleProviderKeyStore) {
      unawaited(reload());
    }
  }

  Future<void> reload({bool forceRefresh = false}) async {
    final generation = ++_requestGeneration;
    final service = widget.serviceClient;
    if (service == null) {
      setState(() => _loadState = _TitleSettingsLoadState.failed);
      return;
    }
    setState(() => _loadState = _TitleSettingsLoadState.loading);
    try {
      final results = await AvailableModelCatalogRepository(
        service: service,
        chatGptApiKeyStore: widget.chatGptApiKeyStore,
        apiCompatibleProviderKeyStore: widget.apiCompatibleProviderKeyStore,
      ).load(forceRefresh: forceRefresh);
      final preference = await widget.settingsPreferences
          .readTitleGenerationPreference();
      if (!mounted || generation != _requestGeneration) return;
      setState(() {
        _models = results.models;
        _preference = _refreshLightweightPreference(preference, results.models);
        _loadState = _TitleSettingsLoadState.loaded;
      });
      if (_preference != preference) {
        await widget.settingsPreferences.writeTitleGenerationPreference(
          _preference,
        );
      }
    } on OpenChatServiceException {
      if (!mounted || generation != _requestGeneration) return;
      setState(() => _loadState = _TitleSettingsLoadState.failed);
    } on ChatGptApiKeyStorageException {
      if (!mounted || generation != _requestGeneration) return;
      setState(() => _loadState = _TitleSettingsLoadState.failed);
    } on ApiCompatibleProviderKeyStorageException {
      if (!mounted || generation != _requestGeneration) return;
      setState(() => _loadState = _TitleSettingsLoadState.failed);
    } on FormatException {
      if (!mounted || generation != _requestGeneration) return;
      setState(() => _loadState = _TitleSettingsLoadState.failed);
    }
  }

  Future<void> _selectMode(String? rawMode) async {
    final mode = switch (rawMode) {
      'conversation' => TitleGenerationMode.conversation,
      'selectedModel' => TitleGenerationMode.selectedModel,
      'lightweight' => TitleGenerationMode.lightweight,
      _ => null,
    };
    if (mode == null || _isSaving) return;
    final model = switch (mode) {
      TitleGenerationMode.conversation => null,
      TitleGenerationMode.selectedModel =>
        _preference.model ??
            (_models.isEmpty ? null : _models.first.toTitlePreference()),
      TitleGenerationMode.lightweight => _lightweightModel(_models),
    };
    if (mode == TitleGenerationMode.selectedModel && model == null) return;
    await _savePreference(TitleGenerationPreference(mode: mode, model: model));
  }

  Future<void> _selectModel(String? routeKey) async {
    if (routeKey == null || _isSaving) return;
    ChatGptModel? selected;
    for (final model in _models) {
      if (model.routeKey == routeKey) {
        selected = model;
        break;
      }
    }
    if (selected == null) return;
    await _savePreference(
      TitleGenerationPreference(
        mode: _preference.mode,
        model: selected.toTitlePreference(),
      ),
    );
  }

  Future<void> _savePreference(TitleGenerationPreference preference) async {
    setState(() => _isSaving = true);
    try {
      await widget.settingsPreferences.writeTitleGenerationPreference(
        preference,
      );
      if (!mounted) return;
      setState(() => _preference = preference);
    } on Object {
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.titlePreferenceSaveFailed,
          type: OpenChatToastType.error,
        );
      }
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    return Card(
      margin: EdgeInsets.zero,
      color: palette.surface,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(OpenChatRadii.card),
        side: BorderSide(color: palette.border),
      ),
      child: ChatGptTitlePreferenceSection(
        preference: _preference,
        models: _models,
        palette: palette,
        isLoading: _loadState == _TitleSettingsLoadState.loading,
        hasError: _loadState == _TitleSettingsLoadState.failed,
        isSaving: _isSaving,
        onRetry: () => unawaited(reload(forceRefresh: true)),
        onModeChanged: _selectMode,
        onModelChanged: _selectModel,
      ),
    );
  }
}

TitleGenerationPreference _refreshLightweightPreference(
  TitleGenerationPreference preference,
  List<ChatGptModel> models,
) {
  if (preference.mode != TitleGenerationMode.lightweight) return preference;
  return TitleGenerationPreference(
    mode: TitleGenerationMode.lightweight,
    model: _lightweightModel(models),
  );
}

DefaultModelPreference? _lightweightModel(List<ChatGptModel> models) {
  final candidates =
      models
          .map((model) => (model: model, size: _parameterCountBillions(model)))
          .where((candidate) => candidate.size != null)
          .toList(growable: false)
        ..sort((left, right) => left.size!.compareTo(right.size!));
  return candidates.isEmpty ? null : candidates.first.model.toTitlePreference();
}

double? _parameterCountBillions(ChatGptModel model) {
  final pattern = RegExp(
    r'(?:^|[-_:./\s])(\d+(?:\.\d+)?)b(?:$|[-_:./\s])',
    caseSensitive: false,
  );
  for (final value in [model.id, model.displayName]) {
    final match = pattern.firstMatch(value);
    final count = double.tryParse(match?.group(1) ?? '');
    if (count != null && count > 0) return count;
  }
  return null;
}

extension on ChatGptModel {
  DefaultModelPreference toTitlePreference() => DefaultModelPreference(
    providerId: providerId,
    modelId: id,
    displayName: displayName,
    connectionId: connectionId,
    workspaceId: workspaceId,
    apiKeyConnectionId:
        providerId == 'chatgpt_api' ||
            ApiCompatibleProviderKeyStore.isApiKeyProviderId(providerId)
        ? connectionId ?? providerId
        : null,
  );
}
