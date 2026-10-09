import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';

import 'support/shad_test_scope.dart';

void main() {
  testWidgets('wide select keeps the trigger wider than its menu', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1068, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        theme: OpenChatTheme.dark,
        home: openChatShadTestScope(
          OpenChatTheme.dark,
          Scaffold(
            body: Padding(
              padding: const EdgeInsets.all(12),
              child: Builder(
                builder: (context) => OpenChatSelect<String>(
                  options: const [
                    OpenChatSelectOption<String>(value: 'one', label: 'One'),
                  ],
                  value: 'one',
                  onChanged: (_) {},
                  palette: OpenChatPalette.of(context),
                  width: double.infinity,
                  menuWidth: 360,
                ),
              ),
            ),
          ),
        ),
      ),
    );

    expect(tester.takeException(), isNull);
    await tester.tap(find.text('One'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });

  testWidgets('labeled select exposes its label and selected value', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    String? selected;

    await tester.pumpWidget(
      MaterialApp(
        theme: OpenChatTheme.light,
        home: openChatShadTestScope(
          OpenChatTheme.light,
          Scaffold(
            body: Builder(
              builder: (context) => Center(
                child: SizedBox(
                  width: 280,
                  child: OpenChatSelectField<String>(
                    label: 'Access mode',
                    options: const <OpenChatSelectOption<String>>[
                      OpenChatSelectOption<String>(
                        value: 'ask',
                        label: 'Ask for approval',
                      ),
                      OpenChatSelectOption<String>(
                        value: 'allow',
                        label: 'Allow',
                      ),
                    ],
                    value: 'ask',
                    palette: OpenChatPalette.of(context),
                    onChanged: (value) => selected = value,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );

    final fieldSemantics = tester.getSemantics(
      find.byType(OpenChatSelectField<String>),
    );
    final fieldData = fieldSemantics.getSemanticsData();
    expect(fieldData.label, contains('Access mode'));
    expect(fieldData.label, contains('Ask for approval'));
    expect(fieldData.flagsCollection.isButton, isTrue);
    expect(fieldData.hasAction(ui.SemanticsAction.tap), isTrue);
    semantics.dispose();

    await tester.tap(find.text('Ask for approval'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Allow').last);
    await tester.pumpAndSettle();

    expect(selected, 'allow');
  });
}
