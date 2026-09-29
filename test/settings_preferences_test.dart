import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';
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
  });

  setUp(() {
    SharedPreferencesAsyncPlatform.instance =
        InMemorySharedPreferencesAsync.empty();
  });

  tearDown(() {
    SharedPreferencesAsyncPlatform.instance = null;
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
      expect(
        await writer.readConversationFont(),
        ConversationFontPreference.manrope,
      );

      await writer.writeConversationWidth(ConversationWidthPreference.wide);
      await writer.writeConversationTextSize(
        ConversationTextSizePreference.large,
      );
      await writer.writeConversationFont(ConversationFontPreference.georgia);

      final reopenedReader = SettingsPreferences(SharedPreferencesAsync());
      expect(
        await reopenedReader.readConversationWidth(),
        ConversationWidthPreference.wide,
      );
      expect(
        await reopenedReader.readConversationTextSize(),
        ConversationTextSizePreference.large,
      );
      expect(
        await reopenedReader.readConversationFont(),
        ConversationFontPreference.georgia,
      );
    },
  );

  test(
    'conversation theme receives the saved width, scale, and font values',
    () {
      final theme = OpenChatTheme.withConversationStyle(
        OpenChatTheme.light,
        maxWidth: ConversationWidthPreference.wide.maxWidth,
        textScale: ConversationTextSizePreference.large.scale,
        fontFamily: ConversationFontPreference.georgia.familyName,
      );

      final style = theme.extension<OpenChatConversationStyle>();
      expect(style?.maxWidth, ConversationWidthPreference.wide.maxWidth);
      expect(style?.textScale, ConversationTextSizePreference.large.scale);
      expect(style?.fontFamily, ConversationFontPreference.georgia.familyName);
    },
  );

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
    expect(find.byIcon(Icons.check_rounded), findsOneWidget);

    await tester.tap(find.text('Wide'));
    await tester.pumpAndSettle();
    expect(selectedValue, 'wide');
    expect(find.text('Wide'), findsOneWidget);
    expect(find.byIcon(Icons.check_rounded), findsNothing);
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

    await tester.tap(find.text('Geniş'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Büyük'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Manrope'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Georgia'));
    await tester.pumpAndSettle();

    expect(
      await preferences.readConversationWidth(),
      ConversationWidthPreference.wide,
    );
    expect(
      await preferences.readConversationTextSize(),
      ConversationTextSizePreference.large,
    );
    expect(
      await preferences.readConversationFont(),
      ConversationFontPreference.georgia,
    );
    final style = Theme.of(tester.element(find.byType(SettingsScreen)))
        .extension<OpenChatConversationStyle>();
    expect(style?.maxWidth, ConversationWidthPreference.wide.maxWidth);
    expect(style?.textScale, ConversationTextSizePreference.large.scale);
    expect(style?.fontFamily, ConversationFontPreference.georgia.familyName);
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
  ConversationFontPreference _font = ConversationFontPreference.manrope;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      locale: const Locale('tr'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: OpenChatTheme.withConversationStyle(
        OpenChatTheme.light,
        maxWidth: _width.maxWidth,
        textScale: _textSize.scale,
        fontFamily: _font.familyName,
      ),
      home: Scaffold(
        body: SettingsScreen(
          themeMode: ThemeMode.light,
          onThemeModeChanged: (_) async {},
          historyStorageStatus: HistoryStorageStatus.available,
          hasConversationHistory: false,
          settingsPreferences: widget.preferences,
          conversationWidth: _width,
          conversationTextSize: _textSize,
          conversationFont: _font,
          onConversationWidthChanged: (value) async {
            await widget.preferences.writeConversationWidth(value);
            if (mounted) setState(() => _width = value);
          },
          onConversationTextSizeChanged: (value) async {
            await widget.preferences.writeConversationTextSize(value);
            if (mounted) setState(() => _textSize = value);
          },
          onConversationFontChanged: (value) async {
            await widget.preferences.writeConversationFont(value);
            if (mounted) setState(() => _font = value);
          },
        ),
      ),
    );
  }
}
