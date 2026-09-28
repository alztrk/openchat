import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:toastification/toastification.dart';

import '../features/chat/data/chat_repository.dart';
import '../features/chat/data/zihora_database.dart';
import '../features/chat/domain/history_storage_status.dart';
import '../features/chat/presentation/chat_screen.dart';
import '../features/settings/data/chat_gpt_api_key_store.dart';
import '../features/settings/data/open_code_api_key_store.dart';
import '../features/settings/data/settings_preferences.dart';
import '../l10n/generated/app_localizations.dart';
import '../platform/windows/zihora_service_client.dart';
import 'zihora_theme.dart';

class ZihoraApp extends StatefulWidget {
  const ZihoraApp({super.key});

  @override
  State<ZihoraApp> createState() => _ZihoraAppState();
}

class _AppRuntime {
  const _AppRuntime(this.database, this.chatRepository);

  final ZihoraDatabase database;
  final ChatRepository chatRepository;
}

class _ZihoraAppState extends State<ZihoraApp> {
  ThemeMode _themeMode = ThemeMode.light;
  Locale? _locale;
  late final ChatGptApiKeyStore _chatGptApiKeyStore;
  late final OpenCodeApiKeyStore _openCodeApiKeyStore;
  late final SettingsPreferences _settingsPreferences;
  late final ZihoraServiceClient _serviceClient;
  late final Future<_AppRuntime> _runtimeReady;
  late final Future<void> _themeModeReady;
  late final Future<void> _localeReady;
  StreamSubscription<ZihoraServiceEvent>? _serviceEventSubscription;

  @override
  void initState() {
    super.initState();
    _serviceClient = ZihoraServiceClient();
    _chatGptApiKeyStore = ChatGptApiKeyStore(FlutterSecureStorage());
    _openCodeApiKeyStore = OpenCodeApiKeyStore(FlutterSecureStorage());
    _settingsPreferences = SettingsPreferences(SharedPreferencesAsync());
    _serviceEventSubscription = _serviceClient.events.listen(
      _handleServiceEvent,
    );
    _runtimeReady = _initializeRuntime();
    _themeModeReady = _loadThemeMode();
    _localeReady = _loadLocale();
  }

  void _handleServiceEvent(ZihoraServiceEvent event) {
    if (event.name != 'chat.title_updated') return;
    final conversationId = event.data['conversationId'];
    final title = event.data['title'];
    if (conversationId is! String || title is! String) return;
    unawaited(_applyGeneratedTitle(conversationId, title));
  }

  Future<void> _applyGeneratedTitle(String conversationId, String title) async {
    try {
      final runtime = await _runtimeReady;
      await runtime.chatRepository.applyGeneratedTitle(
        conversationId: conversationId,
        title: title,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription(
            'while applying a generated conversation title',
          ),
        ),
      );
    }
  }

  Future<_AppRuntime> _initializeRuntime() async {
    ZihoraDatabase? database;
    try {
      String? databasePath;
      if (Platform.isWindows) {
        await _serviceClient.start();
        final health = await _serviceClient.call('system.health');
        final resolvedPath = health['database_path'];
        if (resolvedPath is! String || resolvedPath.isEmpty) {
          throw const ZihoraServiceException(
            code: 'invalid_storage_path',
            message: 'The local service did not provide a database path.',
          );
        }
        databasePath = resolvedPath;
      }

      database = databasePath == null
          ? ZihoraDatabase()
          : ZihoraDatabase.atPath(databasePath);
      await database.customSelect('SELECT 1').getSingle();
      if (Platform.isWindows) {
        final initialization = await _serviceClient.call('system.initialize');
        if (initialization['status'] != 'ready') {
          throw const ZihoraServiceException(
            code: 'storage_initialization_failed',
            message: 'Local conversation storage could not be initialized.',
          );
        }
      }
      _serviceClient.completeDatabaseInitialization();
      return _AppRuntime(database, ChatRepository(database));
    } on Object catch (error, stackTrace) {
      _serviceClient.completeDatabaseInitialization(
        error: error is ZihoraServiceException
            ? error
            : const ZihoraServiceException(
                code: 'storage_initialization_failed',
                message: 'Local conversation storage could not be initialized.',
              ),
      );
      if (database != null) {
        try {
          await database.close();
        } on Object catch (closeError, closeStackTrace) {
          FlutterError.reportError(
            FlutterErrorDetails(
              exception: closeError,
              stack: closeStackTrace,
              library: 'storage',
              context: ErrorDescription(
                'while closing the database after initialization failed',
              ),
            ),
          );
        }
      }
      Error.throwWithStackTrace(error, stackTrace);
    }
  }

  Future<void> _loadThemeMode() async {
    try {
      final themeMode = await _settingsPreferences.readThemeMode();
      if (themeMode != null && mounted) {
        setState(() => _themeMode = themeMode);
      }
    } on PlatformException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading the saved theme mode'),
        ),
      );
    }
  }

  Future<void> _loadLocale() async {
    try {
      final locale = await _settingsPreferences.readLocale();
      if (locale != _locale && mounted) {
        setState(() => _locale = locale);
      }
    } on PlatformException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading the saved app language'),
        ),
      );
    }
  }

  Future<void> _toggleTheme() async {
    await _themeModeReady;
    final platformBrightness =
        WidgetsBinding.instance.platformDispatcher.platformBrightness;
    final isDark = switch (_themeMode) {
      ThemeMode.system => platformBrightness == Brightness.dark,
      ThemeMode.light => false,
      ThemeMode.dark => true,
    };
    await _setThemeMode(isDark ? ThemeMode.light : ThemeMode.dark);
  }

  Future<void> _setThemeMode(ThemeMode themeMode) async {
    await _themeModeReady;
    if (_themeMode == themeMode) return;

    await _settingsPreferences.writeThemeMode(themeMode);
    if (mounted) setState(() => _themeMode = themeMode);
  }

  Future<void> _setLocale(Locale? locale) async {
    await _localeReady;
    if (_locale == locale) return;

    await _settingsPreferences.writeLocale(locale);
    if (mounted) setState(() => _locale = locale);
  }

  @override
  void dispose() {
    unawaited(_disposeRuntime());
    super.dispose();
  }

  Future<void> _disposeRuntime() async {
    await _serviceEventSubscription?.cancel();
    try {
      final runtime = await _runtimeReady;
      await runtime.database.close();
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'storage',
          context: ErrorDescription('while closing the local database'),
        ),
      );
    }

    try {
      await _serviceClient.close();
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'local_service',
          context: ErrorDescription('while stopping the local service'),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return ToastificationWrapper(
      child: MaterialApp(
        title: 'OpenChat',
        debugShowCheckedModeBanner: false,
        theme: ZihoraTheme.light,
        darkTheme: ZihoraTheme.dark,
        themeMode: _themeMode,
        locale: _locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: FutureBuilder<_AppRuntime>(
          future: _runtimeReady,
          builder: (context, snapshot) {
            final runtime = snapshot.data;
            final storageStatus = runtime != null
                ? HistoryStorageStatus.available
                : snapshot.hasError
                ? HistoryStorageStatus.unavailable
                : HistoryStorageStatus.loading;

            return ChatScreen(
              themeMode: _themeMode,
              locale: _locale,
              onThemeModeChanged: _setThemeMode,
              onLocaleChanged: _setLocale,
              onToggleTheme: _toggleTheme,
              settingsPreferences: _settingsPreferences,
              chatRepository: runtime?.chatRepository,
              chatGptApiKeyStore: _chatGptApiKeyStore,
              openCodeApiKeyStore: _openCodeApiKeyStore,
              serviceClient: _serviceClient,
              historyStorageStatus: storageStatus,
            );
          },
        ),
      ),
    );
  }
}
