import 'dart:async';
import 'dart:io';

import 'package:drift/isolate.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sqlite3/sqlite3.dart' as sqlite;
import 'package:toastification/toastification.dart';

import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/chat_attachment_store.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';
import 'package:openchat/features/chat/presentation/chat_screen.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/features/settings/data/chat_gpt_api_key_store.dart';
import 'package:openchat/features/settings/data/open_code_api_key_store.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

import 'package:openchat/app/openchat_text_scaler.dart';
import 'package:openchat/app/openchat_theme.dart';

class OpenChatApp extends StatefulWidget {
  const OpenChatApp({super.key});

  @override
  State<OpenChatApp> createState() => _OpenChatAppState();
}

class _AppRuntime {
  const _AppRuntime(this.database, this.chatRepository);

  final OpenChatDatabase database;
  final ChatRepository chatRepository;
}

class _OpenChatAppState extends State<OpenChatApp> {
  ThemeMode _themeMode = ThemeMode.light;
  Locale? _locale;
  ConversationWidthPreference _conversationWidth =
      ConversationWidthPreference.normal;
  ConversationTextSizePreference _conversationTextSize =
      ConversationTextSizePreference.normal;
  AppFontPreference _appFont = AppFontPreference.manrope;
  late final ChatGptApiKeyStore _chatGptApiKeyStore;
  late final ApiCompatibleProviderKeyStore _apiCompatibleProviderKeyStore;
  late final OpenCodeApiKeyStore _openCodeApiKeyStore;
  late final SettingsPreferences _settingsPreferences;
  late final OpenChatServiceClient _serviceClient;
  late Future<_AppRuntime> _runtimeReady;
  late final Future<void> _themeModeReady;
  late final Future<void> _localeReady;
  late final Future<void> _conversationStyleReady;
  StreamSubscription<OpenChatServiceEvent>? _serviceEventSubscription;

  @override
  void initState() {
    super.initState();
    _serviceClient = OpenChatServiceClient();
    _chatGptApiKeyStore = ChatGptApiKeyStore(FlutterSecureStorage());
    _apiCompatibleProviderKeyStore = ApiCompatibleProviderKeyStore(
      FlutterSecureStorage(),
    );
    _openCodeApiKeyStore = OpenCodeApiKeyStore(FlutterSecureStorage());
    _settingsPreferences = SettingsPreferences(SharedPreferencesAsync());
    _serviceEventSubscription = _serviceClient.events.listen(
      _handleServiceEvent,
    );
    _runtimeReady = _initializeRuntime();
    _themeModeReady = _loadThemeMode();
    _localeReady = _loadLocale();
    _conversationStyleReady = _loadConversationStyle();
  }

  void _handleServiceEvent(OpenChatServiceEvent event) {
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
    const maximumAttempts = 3;
    var attempt = 0;
    String? previousFailureCode;

    while (true) {
      attempt++;
      OpenChatDatabase? database;
      int? chatSchemaVersion;
      var phase = 'service_start';

      try {
        String? databasePath;
        String? storageRoot;
        if (Platform.isWindows) {
          phase = 'database_preflight';
          final localAppData = Platform.environment['LOCALAPPDATA'];
          if (localAppData == null || localAppData.isEmpty) {
            throw const OpenChatServiceException(
              code: 'invalid_storage_path',
              message: 'The local app data directory is unavailable.',
            );
          }
          final preflightDatabasePath =
              '$localAppData${Platform.pathSeparator}OpenChat'
              '${Platform.pathSeparator}db${Platform.pathSeparator}openchat.sqlite3';
          final pendingProfileRestorePath =
              '$localAppData${Platform.pathSeparator}OpenChat'
              '${Platform.pathSeparator}cache${Platform.pathSeparator}'
              'profile-restore${Platform.pathSeparator}restore.pending';
          if (!await File(pendingProfileRestorePath).exists()) {
            await OpenChatDatabase.verifyExistingFile(preflightDatabasePath);
          }

          await _serviceClient.start();
          phase = 'service_health';
          final health = await _serviceClient.call('system.health');
          final resolvedPath = health['database_path'];
          if (resolvedPath is! String || resolvedPath.isEmpty) {
            throw const OpenChatServiceException(
              code: 'invalid_storage_path',
              message: 'The local service did not provide a database path.',
            );
          }
          databasePath = resolvedPath;
          final resolvedStorageRoot = health['storage_root'];
          if (resolvedStorageRoot is! String || resolvedStorageRoot.isEmpty) {
            throw const OpenChatServiceException(
              code: 'invalid_storage_path',
              message: 'The local service did not provide a storage root.',
            );
          }
          storageRoot = resolvedStorageRoot;
          _serviceClient.setStorageLocations(
            databasePath: databasePath,
            storageRoot: storageRoot,
          );
          phase = 'database_prepare';
          final preparation = await _serviceClient.call(
            'system.database.prepare',
            params: <String, Object?>{
              'chatSchemaVersion': OpenChatDatabase.currentSchemaVersion,
            },
          );
          if (preparation['status'] != 'ready') {
            throw const OpenChatServiceException(
              code: 'database_prepare_failed',
              message: 'The local database could not be safely prepared for an update.',
            );
          }
        }

        phase = 'database_open';
        final initializedDatabase = databasePath == null
            ? OpenChatDatabase()
            : OpenChatDatabase.atPath(databasePath);
        database = initializedDatabase;
        await initializedDatabase.customSelect('SELECT 1').getSingle();
        final schemaVersionRow = await initializedDatabase
            .customSelect('PRAGMA user_version')
            .getSingle();
        chatSchemaVersion = schemaVersionRow.read<int>('user_version');
        if (Platform.isWindows) {
          phase = 'service_initialize';
          final initialization = await _serviceClient.call('system.initialize');
          if (initialization['status'] != 'ready') {
            throw const OpenChatServiceException(
              code: 'storage_initialization_failed',
              message: 'Local conversation storage could not be initialized.',
            );
          }
        }

        _serviceClient.completeDatabaseInitialization();
        await _writeRuntimeDiagnostic(
          event: attempt > 1
              ? 'runtime_initialization_recovered'
              : 'runtime_initialization_succeeded',
          phase: phase,
          attempt: attempt,
          code: previousFailureCode ?? 'none',
          sqliteVersion: sqlite.sqlite3.version.libVersion,
          sqliteVersionNumber: sqlite.sqlite3.version.versionNumber,
          chatSchemaVersion: chatSchemaVersion,
        );
        return _AppRuntime(
          initializedDatabase,
          ChatRepository(
            initializedDatabase,
            attachmentStore: storageRoot == null
                ? null
                : ChatAttachmentStore(storageRoot),
          ),
        );
      } on Object catch (error, stackTrace) {
        final failure = error is OpenChatServiceException
            ? error
            : const OpenChatServiceException(
                code: 'storage_initialization_failed',
                message: 'Local conversation storage could not be initialized.',
              );
        previousFailureCode = _startupFailureCode(error);
        await _writeRuntimeDiagnostic(
          event: 'runtime_initialization_failed',
          phase: phase,
          attempt: attempt,
          code: previousFailureCode,
          sqliteVersion: sqlite.sqlite3.version.libVersion,
          sqliteVersionNumber: sqlite.sqlite3.version.versionNumber,
          chatSchemaVersion: chatSchemaVersion,
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

        if (attempt < maximumAttempts && _isRetryableRuntimeFailure(error)) {
          try {
            await _serviceClient.restartAfterInitializationFailure();
          } on Object catch (restartError, restartStackTrace) {
            final restartFailure = restartError is OpenChatServiceException
                ? restartError
                : const OpenChatServiceException(
                    code: 'service_restart_failed',
                    message: 'The local service could not be restarted.',
                    retryable: true,
                  );
            await _writeRuntimeDiagnostic(
              event: 'runtime_recovery_failed',
              phase: 'service_restart',
              attempt: attempt,
              code: restartFailure.code,
            );
            _serviceClient.completeDatabaseInitialization(
              error: restartFailure,
            );
            Error.throwWithStackTrace(restartError, restartStackTrace);
          }
          await Future<void>.delayed(Duration(milliseconds: 300 * attempt));
          continue;
        }

        _serviceClient.completeDatabaseInitialization(error: failure);
        Error.throwWithStackTrace(error, stackTrace);
      }
    }
  }

  String _startupFailureCode(Object error) {
    if (error is DatabaseIntegrityFailure) return error.code;
    if (error is OpenChatServiceException) return error.code;
    if (error is! DriftRemoteException) return error.runtimeType.toString();

    final cause = error.remoteCause;
    final description = cause.toString();
    final sqliteError = RegExp(r'\bSqliteException\((\d+)(?:,\s*(\d+))?\)')
        .firstMatch(description);
    final category = _startupFailureCategory(description);
    if (sqliteError != null) {
      return 'drift_sqlite_${sqliteError.group(1)}_${sqliteError.group(2) ?? 'unknown'}_$category';
    }

    final causeType = cause.runtimeType.toString().replaceAll(
      RegExp(r'[^a-zA-Z0-9_-]'),
      '_',
    );
    return 'drift_remote_${causeType}_$category';
  }

  String _startupFailureCategory(String description) {
    final message = description.toLowerCase();
    return switch (true) {
      _ when message.contains('failed to load dynamic library') =>
        'native_library_load',
      _ when message.contains('database is locked') => 'database_locked',
      _ when message.contains('unable to open database') =>
        'database_open_denied',
      _ when message.contains('disk i/o error') => 'disk_io_error',
      _ when message.contains('no such table') => 'missing_table',
      _ when message.contains('no such column') => 'missing_column',
      _ when message.contains('already exists') => 'object_already_exists',
      _ when message.contains('not a database') => 'not_a_database',
      _ when message.contains('malformed') => 'malformed_database',
      _ when message.contains('readonly') || message.contains('read-only') =>
        'read_only',
      _ => 'other',
    };
  }

  bool _isRetryableRuntimeFailure(Object error) {
    if (error is DatabaseIntegrityFailure) return error.retryable;
    if (error is OpenChatServiceException) {
      return error.code != 'invalid_storage_path' &&
          error.code != 'service_executable_missing' &&
          error.code != 'database_corrupt' &&
          error.code != 'database_prepare_failed';
    }
    if (_isCorruptDatabaseFailure(error)) return false;
    return true;
  }

  bool _isCorruptDatabaseFailure(Object? error) {
    if (error is DatabaseIntegrityFailure) return error.isCorrupt;
    if (error is OpenChatServiceException) {
      return error.code == 'database_corrupt';
    }
    if (error is! DriftRemoteException) return false;

    final description = error.remoteCause.toString().toLowerCase();
    if (description.contains('malformed') ||
        description.contains('not a database')) {
      return true;
    }
    return RegExp(r'sqliteexception\((11|26)(?:,|\))').hasMatch(description);
  }

  HistoryStorageStatus _historyStorageStatusFor(Object? error) {
    if (_isCorruptDatabaseFailure(error)) return HistoryStorageStatus.corrupt;
    if (error is OpenChatServiceException) {
      if (error.code == 'database_prepare_failed') {
        return HistoryStorageStatus.backupUnavailable;
      }
    }
    return HistoryStorageStatus.unavailable;
  }

  bool _canRetryStorageManually(Object? error) {
    if (_isCorruptDatabaseFailure(error)) return false;
    if (error is DatabaseIntegrityFailure) return error.retryable;
    if (error is OpenChatServiceException) {
      return error.code != 'invalid_storage_path' &&
          error.code != 'service_executable_missing';
    }
    return true;
  }

  Future<void> _writeRuntimeDiagnostic({
    required String event,
    required String phase,
    required int attempt,
    required String code,
    String? sqliteVersion,
    int? sqliteVersionNumber,
    int? chatSchemaVersion,
  }) async {
    if (!Platform.isWindows) return;

    final localAppData = Platform.environment['LOCALAPPDATA'];
    if (localAppData == null || localAppData.isEmpty) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: StateError('The local app data directory is unavailable.'),
          library: 'runtime_startup',
        ),
      );
      return;
    }

    try {
      final logDirectory = Directory(
        '$localAppData${Platform.pathSeparator}OpenChat${Platform.pathSeparator}logs',
      );
      await logDirectory.create(recursive: true);
      final logFile = File(
        '${logDirectory.path}${Platform.pathSeparator}openchat-app.log',
      );
      final safeCode = code.replaceAll(RegExp(r'[^a-zA-Z0-9_-]'), '_');
      final safeSqliteVersion = sqliteVersion?.replaceAll(
        RegExp(r'[^a-zA-Z0-9._-]'),
        '_',
      );
      final sqliteVersionField = safeSqliteVersion == null
          ? ''
          : ' dart_sqlite_version=$safeSqliteVersion';
      final sqliteVersionNumberField = sqliteVersionNumber == null
          ? ''
          : ' dart_sqlite_version_number=$sqliteVersionNumber';
      final chatSchemaVersionField = chatSchemaVersion == null
          ? ''
          : ' chat_schema_version=$chatSchemaVersion';
      final timestamp = DateTime.now().millisecondsSinceEpoch;
      await logFile.writeAsString(
        'timestamp_unix_ms=$timestamp component=app event=$event phase=$phase attempt=$attempt code=$safeCode$sqliteVersionField$sqliteVersionNumberField$chatSchemaVersionField\n',
        mode: FileMode.append,
        flush: true,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'runtime_startup',
          context: ErrorDescription('while recording startup diagnostics'),
        ),
      );
    }
  }

  void _retryRuntimeInitialization() {
    _serviceClient.resetDatabaseInitialization();
    setState(() => _runtimeReady = _initializeRuntime());
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

  Future<void> _loadConversationStyle() async {
    try {
      final (width, textSize, font) = await (
        _settingsPreferences.readConversationWidth(),
        _settingsPreferences.readConversationTextSize(),
        _settingsPreferences.readAppFont(),
      ).wait;
      if (!mounted) return;
      setState(() {
        _conversationWidth = width;
        _conversationTextSize = textSize;
        _appFont = font;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading conversation appearance'),
        ),
      );
    }
  }

  Future<void> _setConversationWidth(ConversationWidthPreference width) async {
    await _conversationStyleReady;
    if (_conversationWidth == width) return;
    await _settingsPreferences.writeConversationWidth(width);
    if (mounted) setState(() => _conversationWidth = width);
  }

  Future<void> _setConversationTextSize(
    ConversationTextSizePreference size,
  ) async {
    await _conversationStyleReady;
    if (_conversationTextSize == size) return;
    await _settingsPreferences.writeConversationTextSize(size);
    if (mounted) setState(() => _conversationTextSize = size);
  }

  Future<void> _setAppFont(AppFontPreference font) async {
    await _conversationStyleReady;
    if (_appFont == font) return;
    await _settingsPreferences.writeAppFont(font);
    if (mounted) setState(() => _appFont = font);
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
    final lightTheme = OpenChatTheme.withConversationStyle(
      OpenChatTheme.light,
      maxWidth: _conversationWidth.maxWidth,
      fontFamily: _appFont.familyName,
    );
    final darkTheme = OpenChatTheme.withConversationStyle(
      OpenChatTheme.dark,
      maxWidth: _conversationWidth.maxWidth,
      fontFamily: _appFont.familyName,
    );

    return ToastificationWrapper(
      child: MaterialApp(
        title: 'OpenChat',
        debugShowCheckedModeBanner: false,
        theme: lightTheme,
        darkTheme: darkTheme,
        themeMode: _themeMode,
        locale: _locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        builder: (context, child) {
          final mediaQuery = MediaQuery.of(context);
          return MediaQuery(
            data: mediaQuery.copyWith(
              textScaler: OpenChatTextScaler(
                mediaQuery.textScaler,
                _conversationTextSize.scale,
              ),
            ),
            child: child ?? const SizedBox.shrink(),
          );
        },
        home: FutureBuilder<_AppRuntime>(
          future: _runtimeReady,
          builder: (context, snapshot) {
            final runtime = snapshot.data;
            final storageStatus = runtime != null
                ? HistoryStorageStatus.available
                : snapshot.hasError
                ? _historyStorageStatusFor(snapshot.error)
                : HistoryStorageStatus.loading;

            return ChatScreen(
              themeMode: _themeMode,
              locale: _locale,
              conversationWidth: _conversationWidth,
              conversationTextSize: _conversationTextSize,
              appFont: _appFont,
              onThemeModeChanged: _setThemeMode,
              onLocaleChanged: _setLocale,
              onConversationWidthChanged: _setConversationWidth,
              onConversationTextSizeChanged: _setConversationTextSize,
              onAppFontChanged: _setAppFont,
              onToggleTheme: _toggleTheme,
              settingsPreferences: _settingsPreferences,
              chatRepository: runtime?.chatRepository,
              chatGptApiKeyStore: _chatGptApiKeyStore,
              apiCompatibleProviderKeyStore: _apiCompatibleProviderKeyStore,
              openCodeApiKeyStore: _openCodeApiKeyStore,
              serviceClient: _serviceClient,
              onRetryStorage: snapshot.hasError
                  ? _canRetryStorageManually(snapshot.error)
                        ? _retryRuntimeInitialization
                        : null
                  : null,
              historyStorageStatus: storageStatus,
            );
          },
        ),
      ),
    );
  }
}
