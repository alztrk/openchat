import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_text_scaler.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/default_model_preference.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';
import 'package:openchat/features/chat/domain/history_search_result.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_pane.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/presentation/settings_screen.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final iconFont = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await iconFont.load();
    final fontLoader = FontLoader('Manrope')
      ..addFont(rootBundle.load('assets/fonts/Manrope[wght].ttf'));
    await fontLoader.load();
    final uiFontLoader = FontLoader(OpenChatTypography.uiFontFamily)
      ..addFont(rootBundle.load('assets/fonts/SourceSans3VF-Upright.ttf'));
    await uiFontLoader.load();
    final codeFontLoader = FontLoader(OpenChatTypography.codeFontFamily)
      ..addFont(rootBundle.load('assets/fonts/SourceCodeVF-Upright.ttf'));
    await codeFontLoader.load();
  });

  setUp(() {
    SharedPreferencesAsyncPlatform.instance =
        InMemorySharedPreferencesAsync.empty();
  });

  tearDown(() {
    SharedPreferencesAsyncPlatform.instance = null;
  });

  test('saved history searches persist their filters and date range', () async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    const saved = SavedHistorySearch(
      name: 'Recent provider errors',
      query: 'connection failed',
      filters: HistorySearchFilters(
        providerId: 'mistral',
        projectId: 'project-1',
        isArchived: false,
        tag: 'Research',
      ),
      fromUnixMs: 1791417600000,
      throughUnixMs: 1791590400000,
    );

    await preferences.writeSavedHistorySearches(const <SavedHistorySearch>[
      saved,
    ]);

    final reopened = await SettingsPreferences(SharedPreferencesAsync())
        .readSavedHistorySearches();
    expect(reopened, hasLength(1));
    expect(reopened.single.toJson(), saved.toJson());
  });

  test(
    'appearance preferences use defaults and survive a new preferences reader',
    () async {
      final writer = SettingsPreferences(SharedPreferencesAsync());

      expect(
        await writer.readConversationWidth(),
        ConversationWidthPreference.normal,
      );
      expect(
        await writer.readConversationTextSize(),
        ConversationTextSizePreference.normal,
      );
      expect(await writer.readAppFont(), AppFontPreference.sourceSans3);

      await writer.writeConversationWidth(ConversationWidthPreference.wide);
      await writer.writeConversationTextSize(
        ConversationTextSizePreference.large,
      );
      await writer.writeAppFont(AppFontPreference.georgia);

      final reopenedReader = SettingsPreferences(SharedPreferencesAsync());
      expect(
        await reopenedReader.readConversationWidth(),
        ConversationWidthPreference.wide,
      );
      expect(
        await reopenedReader.readConversationTextSize(),
        ConversationTextSizePreference.large,
      );
      expect(await reopenedReader.readAppFont(), AppFontPreference.georgia);
    },
  );

  test('all supported locale preferences persist and restore', () async {
    final writer = SettingsPreferences(SharedPreferencesAsync());
    final reader = SettingsPreferences(SharedPreferencesAsync());
    const locales = <Locale?>[
      null,
      Locale('en'),
      Locale('tr'),
      Locale('es'),
      Locale('de'),
      Locale('fr'),
    ];

    for (final locale in locales) {
      await writer.writeLocale(locale);
      expect(await reader.readLocale(), locale);
    }
  });

  test(
    'unknown saved locale falls back to system without becoming supported',
    () async {
      final preferences = SharedPreferencesAsync();
      await preferences.setString('appearance.locale', 'xx');

      final reader = SettingsPreferences(preferences);
      expect(await reader.readLocale(), isNull);
    },
  );

  test('default model preference and hidden model keys persist', () async {
    final writer = SettingsPreferences(SharedPreferencesAsync());

    expect(await writer.readDefaultModel(), isNull);
    expect(await writer.readHiddenModelKeys(), isEmpty);

    const preference = DefaultModelPreference(
      providerId: 'opencode',
      modelId: 'big-pickle',
      displayName: 'Big Pickle',
    );
    await writer.writeDefaultModel(preference);
    await writer.writeHiddenModelKeys({
      'opencode:::mimo-v2.5-free',
      'opencode:::space-bunny-free',
    });

    final reopenedReader = SettingsPreferences(SharedPreferencesAsync());
    final savedModel = await reopenedReader.readDefaultModel();
    expect(savedModel, isNotNull);
    expect(savedModel?.providerId, 'opencode');
    expect(savedModel?.modelId, 'big-pickle');
    expect(savedModel?.displayName, 'Big Pickle');
    expect(savedModel?.routeKey, 'opencode:::big-pickle');

    final savedHidden = await reopenedReader.readHiddenModelKeys();
    expect(savedHidden, contains('opencode:::mimo-v2.5-free'));
    expect(savedHidden, contains('opencode:::space-bunny-free'));
    expect(savedHidden.length, 2);

    await writer.writeDefaultModel(null);
    expect(await reopenedReader.readDefaultModel(), isNull);

    await writer.writeHiddenModelKeys({});
    expect(await reopenedReader.readHiddenModelKeys(), isEmpty);
  });

  test(
    'local engine model directories persist and reset to defaults',
    () async {
      final writer = SettingsPreferences(SharedPreferencesAsync());
      final reader = SettingsPreferences(SharedPreferencesAsync());

      expect(await reader.readLocalModelDirectory('llama_cpp'), isNull);
      await writer.writeLocalModelDirectory(
        'llama_cpp',
        r'E:\LocalModels\Llama',
      );
      expect(
        await reader.readLocalModelDirectory('llama_cpp'),
        r'E:\LocalModels\Llama',
      );

      await writer.writeLocalModelDirectory('llama_cpp', null);
      expect(await reader.readLocalModelDirectory('llama_cpp'), isNull);
      await expectLater(
        reader.readLocalModelDirectory('unknown'),
        throwsArgumentError,
      );
    },
  );

  test('shared instructions and tool permission mode persist', () async {
    final writer = SettingsPreferences(SharedPreferencesAsync());
    expect(await writer.readSharedInstructions(), isEmpty);
    expect(
      await writer.readToolPermissionMode(),
      ToolPermissionMode.requireApproval,
    );
    expect(ToolPermissionMode.requireApproval.serviceValue, 'require_approval');
    expect(ToolPermissionMode.fullAccess.serviceValue, 'full_access');

    await writer.writeSharedInstructions('Keep answers concise.');
    await writer.writeToolPermissionMode(ToolPermissionMode.fullAccess);

    final reader = SettingsPreferences(SharedPreferencesAsync());
    expect(await reader.readSharedInstructions(), 'Keep answers concise.');
    expect(
      await reader.readToolPermissionMode(),
      ToolPermissionMode.fullAccess,
    );
  });

  test('project tool permission rules persist separately by project', () async {
    final preferences = SharedPreferencesAsync();
    final writer = SettingsPreferences(preferences);
    expect(await writer.readProjectToolPermissionRules('project-1'), isEmpty);

    await writer.writeProjectToolPermissionRules('project-1', {
      'execute_command': ToolPermissionRule.ask,
      'write_file': ToolPermissionRule.deny,
      'run_project_task__verify': ToolPermissionRule.deny,
      'project_tool__lint': ToolPermissionRule.allow,
      'mcp__local_docs__*': ToolPermissionRule.allow,
      'mcp__local_docs__read-file': ToolPermissionRule.deny,
      'read_file': ToolPermissionRule.inherit,
    });
    await writer.writeProjectToolPermissionRules('project-2', {
      'read_file': ToolPermissionRule.allow,
    });

    final reader = SettingsPreferences(SharedPreferencesAsync());
    expect(await reader.readProjectToolPermissionRules('project-1'), {
      'execute_command': ToolPermissionRule.ask,
      'write_file': ToolPermissionRule.deny,
      'run_project_task__verify': ToolPermissionRule.deny,
      'project_tool__lint': ToolPermissionRule.allow,
      'mcp__local_docs__*': ToolPermissionRule.allow,
      'mcp__local_docs__read-file': ToolPermissionRule.deny,
    });
    expect(await reader.readProjectToolPermissionRules('project-2'), {
      'read_file': ToolPermissionRule.allow,
    });

    await writer.writeProjectToolPermissionRules('project-1', const {});
    expect(await reader.readProjectToolPermissionRules('project-1'), isEmpty);
    await expectLater(
      writer.writeProjectToolPermissionRules('project-1', {
        'unknown_tool': ToolPermissionRule.allow,
      }),
      throwsArgumentError,
    );
  });

  test(
    'MCP tool permission rules persist up to the configured server limit',
    () async {
      final preferences = SettingsPreferences(SharedPreferencesAsync());
      final rules = <String, ToolPermissionRule>{};
      for (var server = 0; server < 8; server++) {
        rules['mcp__server_${server}__*'] = ToolPermissionRule.ask;
        for (var tool = 0; tool < 128; tool++) {
          rules['mcp__server_${server}__tool_$tool'] = ToolPermissionRule.deny;
        }
      }
      for (final tool in projectToolRuleNames) {
        rules[tool] = ToolPermissionRule.ask;
      }
      for (var task = 0; task < 32; task++) {
        rules['run_project_task__task_$task'] = ToolPermissionRule.deny;
      }
      rules['mcp__legacy_server__*'] = ToolPermissionRule.allow;

      expect(rules, hasLength(1080));
      await preferences.writeProjectToolPermissionRules('project-large', rules);
      expect(
        await preferences.readProjectToolPermissionRules('project-large'),
        rules,
      );

      rules['mcp__overflow_server__*'] = ToolPermissionRule.allow;
      await expectLater(
        preferences.writeProjectToolPermissionRules('project-overflow', rules),
        throwsArgumentError,
      );
    },
  );

  test('appearance theme applies the saved width and app font', () {
    final theme = OpenChatTheme.withConversationStyle(
      OpenChatTheme.light,
      maxWidth: ConversationWidthPreference.wide.maxWidth,
      fontFamily: AppFontPreference.georgia.familyName,
    );

    final style = theme.extension<OpenChatConversationStyle>();
    expect(style?.maxWidth, ConversationWidthPreference.wide.maxWidth);
    expect(style?.fontFamily, AppFontPreference.georgia.familyName);
    expect(
      theme.textTheme.bodyLarge?.fontFamily,
      AppFontPreference.georgia.familyName,
    );
  });

  test('app text size composes with the platform accessibility scale', () {
    const scaler = OpenChatTextScaler(TextScaler.linear(1.5), 1.25);

    expect(scaler.scale(14), 26.25);
    expect(scaler.textScaleFactor, 1.875);
  });

  testWidgets('shared select updates its label and closes after selection', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(800, 600);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    var selectedValue = 'normal';
    await tester.pumpWidget(
      MaterialApp(
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: Center(
            child: StatefulBuilder(
              builder: (context, setState) => OpenChatSelect<String>(
                options: const [
                  OpenChatSelectOption<String>(
                    value: 'normal',
                    label: 'Normal',
                  ),
                  OpenChatSelectOption<String>(value: 'wide', label: 'Wide'),
                ],
                value: selectedValue,
                onChanged: (value) => setState(() => selectedValue = value),
                palette: OpenChatPalette.of(context),
                width: 240,
              ),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Normal'));
    await tester.pumpAndSettle();
    expect(find.text('Wide'), findsOneWidget);
    expect(find.byIcon(LucideIcons.check), findsOneWidget);

    await tester.tap(find.text('Wide'));
    await tester.pumpAndSettle();
    expect(selectedValue, 'wide');
    expect(find.text('Wide'), findsOneWidget);
    expect(find.byIcon(LucideIcons.check), findsNothing);
  });

  testWidgets('appearance controls apply and persist all three choices', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final preferences = SettingsPreferences(SharedPreferencesAsync());

    await tester.pumpWidget(_SettingsTestHost(preferences: preferences));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Görünüm'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Türkçe'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('İngilizce'));
    await tester.pumpAndSettle();
    expect(find.text('Appearance'), findsWidgets);
    await tester.tap(find.text('Dark'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Wide'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Large'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Manrope'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Georgia'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('English'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Spanish'));
    await tester.pumpAndSettle();
    expect(await preferences.readLocale(), const Locale('es'));

    expect(
      await preferences.readConversationWidth(),
      ConversationWidthPreference.wide,
    );
    expect(
      await preferences.readConversationTextSize(),
      ConversationTextSizePreference.large,
    );
    expect(await preferences.readAppFont(), AppFontPreference.georgia);
    expect(await preferences.readThemeMode(), ThemeMode.dark);
    expect(await preferences.readLocale(), const Locale('es'));
    final style = Theme.of(tester.element(find.byType(SettingsScreen)))
        .extension<OpenChatConversationStyle>();
    expect(
      Theme.of(tester.element(find.byType(SettingsScreen))).brightness,
      Brightness.dark,
    );
    expect(style?.maxWidth, ConversationWidthPreference.wide.maxWidth);
    expect(style?.fontFamily, AppFontPreference.georgia.familyName);
    expect(
      MediaQuery.textScalerOf(tester.element(find.byType(SettingsScreen)))
          .scale(14),
      14 * ConversationTextSizePreference.large.scale,
    );
    final theme = Theme.of(tester.element(find.byType(SettingsScreen)));
    expect(
      theme.textTheme.bodyLarge?.fontFamily,
      AppFontPreference.georgia.familyName,
    );
    expect(
      tester
          .widget<Text>(find.byKey(const ValueKey('app-font-probe')))
          .style
          ?.fontFamily,
      AppFontPreference.georgia.familyName,
    );
  });

  testWidgets('shared instructions entered in settings are saved', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final preferences = SettingsPreferences(SharedPreferencesAsync());

    await tester.pumpWidget(_SettingsTestHost(preferences: preferences));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Ortak talimatlar'));
    await tester.pumpAndSettle();
    expect(
      find.text(
        "Bu talimatlar bağlı olan tüm sağlayıcılara her sohbette gönderilir. Dosya araçlarının erişimi Araç erişimi ayarına uyar.",
      ),
      findsOneWidget,
    );
    final instructionsField = find.byWidgetPredicate(
      (widget) =>
          widget is TextField &&
          widget.decoration?.hintText ==
              'Yanıtların nasıl verilmesini istediğini yaz...',
    );
    await tester.enterText(instructionsField, 'Keep answers concise.');
    await tester.pump();
    expect(
      tester.widget<TextField>(instructionsField).controller?.text,
      'Keep answers concise.',
    );
    expect(
      tester
          .widget<FilledButton>(
            find.ancestor(
              of: find.text('Kaydet'),
              matching: find.byType(FilledButton),
            ),
          )
          .onPressed,
      isNotNull,
    );
    await tester.tap(find.text('Kaydet'));
    await tester.pumpAndSettle();
    await tester.pump(const Duration(seconds: 5));

    expect(await preferences.readSharedInstructions(), 'Keep answers concise.');
  });

  testWidgets('conversation width and text size reach rendered messages', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final messageController = TextEditingController();
    addTearDown(messageController.dispose);
    var textSize = ConversationTextSizePreference.normal;
    late StateSetter updateTextSize;

    await tester.pumpWidget(
      StatefulBuilder(
        builder: (context, setState) {
          updateTextSize = setState;
          return MaterialApp(
            locale: const Locale('tr'),
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            theme: OpenChatTheme.withConversationStyle(
              OpenChatTheme.light,
              maxWidth: ConversationWidthPreference.wide.maxWidth,
              fontFamily: AppFontPreference.georgia.familyName,
            ),
            builder: (context, child) {
              final mediaQuery = MediaQuery.of(context);
              return MediaQuery(
                data: mediaQuery.copyWith(
                  textScaler: OpenChatTextScaler(
                    mediaQuery.textScaler,
                    textSize.scale,
                  ),
                ),
                child: child ?? const SizedBox.shrink(),
              );
            },
            home: Scaffold(
              body: Column(
                children: [
                  Text(
                    'Application appearance probe',
                    style: Theme.of(context).textTheme.bodyLarge,
                  ),
                  Expanded(
                    child: ConversationPane(
                      messageController: messageController,
                      showHistoryButton: false,
                      onOpenHistory: () {},
                      onSendMessage: () {},
                      providerId: 'chatgpt',
                      messages: const [
                        ChatMessage(
                          id: 'appearance-user',
                          role: ChatMessageRole.user,
                          content: 'User appearance probe',
                        ),
                        ChatMessage(
                          id: 'appearance-assistant',
                          role: ChatMessageRole.assistant,
                          content: 'Assistant appearance probe',
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          );
        },
      ),
    );
    await tester.pumpAndSettle();

    final userText = tester.widget<Text>(find.text('User appearance probe'));
    final assistantText = find.text(
      'Assistant appearance probe',
      findRichText: true,
    );
    final normalAssistantText = tester.widget<RichText>(assistantText);
    expect(normalAssistantText.text.style?.fontSize, 14);
    expect(userText.style?.fontSize, 15);
    expect(userText.style?.fontFamily, AppFontPreference.georgia.familyName);
    final applicationText = tester.widget<RichText>(
      find.byWidgetPredicate(
        (widget) =>
            widget is RichText &&
            widget.text.toPlainText() == 'Application appearance probe',
      ),
    );
    expect(applicationText.textScaler.scale(15), 15);
    final normalUserRichText = tester.widget<RichText>(
      find.byWidgetPredicate(
        (widget) =>
            widget is RichText && widget.text.toPlainText() == userText.data,
      ),
    );
    expect(normalUserRichText.textScaler.scale(15), 15);
    updateTextSize(() => textSize = ConversationTextSizePreference.large);
    await tester.pumpAndSettle();

    final largeUserText = tester.widget<Text>(
      find.text('User appearance probe'),
    );
    expect(largeUserText.style?.fontSize, 15);
    expect(
      largeUserText.style?.fontFamily,
      AppFontPreference.georgia.familyName,
    );
    expect(
      tester.widget<RichText>(assistantText).text.style?.fontSize,
      14 * ConversationTextSizePreference.large.scale,
    );
    expect(
      tester
          .widget<RichText>(
            find.byWidgetPredicate(
              (widget) =>
                  widget is RichText &&
                  widget.text.toPlainText() == 'Application appearance probe',
            ),
          )
          .textScaler
          .scale(15),
      15 * ConversationTextSizePreference.large.scale,
    );
    expect(
      tester
          .widget<RichText>(
            find.byWidgetPredicate(
              (widget) =>
                  widget is RichText &&
                  widget.text.toPlainText() == 'User appearance probe',
            ),
          )
          .textScaler
          .scale(15),
      15 * ConversationTextSizePreference.large.scale,
    );

    updateTextSize(() => textSize = ConversationTextSizePreference.small);
    await tester.pumpAndSettle();

    expect(
      tester.widget<RichText>(assistantText).text.style?.fontSize,
      14 * ConversationTextSizePreference.small.scale,
    );
    expect(
      tester
          .widget<RichText>(
            find.byWidgetPredicate(
              (widget) =>
                  widget is RichText &&
                  widget.text.toPlainText() == 'Application appearance probe',
            ),
          )
          .textScaler
          .scale(15),
      15 * ConversationTextSizePreference.small.scale,
    );

    final messageWidthConstraints = tester
        .widgetList<ConstrainedBox>(
          find.ancestor(
            of: find.text('User appearance probe'),
            matching: find.byType(ConstrainedBox),
          ),
        )
        .map((box) => box.constraints.maxWidth);
    expect(
      messageWidthConstraints,
      contains(ConversationWidthPreference.wide.maxWidth),
    );
  });
}

class _SettingsTestHost extends StatefulWidget {
  const _SettingsTestHost({required this.preferences});

  final SettingsPreferences preferences;

  @override
  State<_SettingsTestHost> createState() => _SettingsTestHostState();
}

class _SettingsTestHostState extends State<_SettingsTestHost> {
  ConversationWidthPreference _width = ConversationWidthPreference.normal;
  ConversationTextSizePreference _textSize =
      ConversationTextSizePreference.normal;
  AppFontPreference _appFont = AppFontPreference.manrope;
  ThemeMode _themeMode = ThemeMode.light;
  Locale? _locale = const Locale('tr');

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      locale: _locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: OpenChatTheme.withConversationStyle(
        OpenChatTheme.light,
        maxWidth: _width.maxWidth,
        fontFamily: _appFont.familyName,
      ),
      darkTheme: OpenChatTheme.withConversationStyle(
        OpenChatTheme.dark,
        maxWidth: _width.maxWidth,
        fontFamily: _appFont.familyName,
      ),
      themeMode: _themeMode,
      builder: (context, child) {
        final mediaQuery = MediaQuery.of(context);
        return MediaQuery(
          data: mediaQuery.copyWith(
            textScaler: OpenChatTextScaler(
              mediaQuery.textScaler,
              _textSize.scale,
            ),
          ),
          child: child ?? const SizedBox.shrink(),
        );
      },
      home: Scaffold(
        body: Column(
          children: [
            Expanded(
              child: SettingsScreen(
                themeMode: _themeMode,
                locale: _locale,
                onThemeModeChanged: (value) async {
                  await widget.preferences.writeThemeMode(value);
                  if (mounted) setState(() => _themeMode = value);
                },
                onLocaleChanged: (value) async {
                  await widget.preferences.writeLocale(value);
                  if (mounted) setState(() => _locale = value);
                },
                historyStorageStatus: HistoryStorageStatus.available,
                hasConversationHistory: false,
                settingsPreferences: widget.preferences,
                conversationWidth: _width,
                conversationTextSize: _textSize,
                appFont: _appFont,
                onConversationWidthChanged: (value) async {
                  await widget.preferences.writeConversationWidth(value);
                  if (mounted) setState(() => _width = value);
                },
                onConversationTextSizeChanged: (value) async {
                  await widget.preferences.writeConversationTextSize(value);
                  if (mounted) setState(() => _textSize = value);
                },
                onAppFontChanged: (value) async {
                  await widget.preferences.writeAppFont(value);
                  if (mounted) setState(() => _appFont = value);
                },
              ),
            ),
            Builder(
              builder: (context) => Text(
                'Typography probe',
                key: const ValueKey('app-font-probe'),
                style: Theme.of(context).textTheme.bodyLarge,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
