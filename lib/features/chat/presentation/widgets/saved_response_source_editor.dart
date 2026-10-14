import 'package:flutter/material.dart';
import 'package:flutter_code_editor/flutter_code_editor.dart';
import 'package:highlight/highlight_core.dart' show Mode;
import 'package:highlight/languages/css.dart' as css_language;
import 'package:highlight/languages/dart.dart' as dart_language;
import 'package:highlight/languages/go.dart' as go_language;
import 'package:highlight/languages/javascript.dart' as javascript_language;
import 'package:highlight/languages/json.dart' as json_language;
import 'package:highlight/languages/markdown.dart' as markdown_language;
import 'package:highlight/languages/python.dart' as python_language;
import 'package:highlight/languages/rust.dart' as rust_language;
import 'package:highlight/languages/sql.dart' as sql_language;
import 'package:highlight/languages/typescript.dart' as typescript_language;
import 'package:highlight/languages/xml.dart' as xml_language;
import 'package:highlight/languages/yaml.dart' as yaml_language;

import 'package:openchat/app/openchat_theme.dart';

CodeController createSavedResponseSourceController(String content) =>
    _SavedResponseCodeController(text: content);

class _SavedResponseCodeController extends CodeController {
  _SavedResponseCodeController({required String text})
    : super(
        text: text,
        language: markdown_language.markdown,
        analyzer: const DefaultLocalAnalyzer(),
        modifiers: const [],
      );

  @override
  set value(TextEditingValue newValue) {
    if (newValue.text == text) {
      super.value = newValue;
      return;
    }

    fullText = newValue.text;
    super.value = TextEditingValue(
      text: text,
      selection: newValue.selection,
      composing: newValue.composing,
    );
  }
}

class SavedResponseSourceEditor extends StatelessWidget {
  const SavedResponseSourceEditor({
    required this.controller,
    required this.languageId,
    required this.sourceLabel,
    required this.languageLabel,
    required this.onLanguageChanged,
    required this.onChanged,
    super.key,
  });

  final CodeController controller;
  final String languageId;
  final String sourceLabel;
  final String languageLabel;
  final ValueChanged<String> onLanguageChanged;
  final ValueChanged<String> onChanged;

  static final Map<String, ({String label, Mode mode})> _languages = {
    'markdown': (label: 'Markdown', mode: markdown_language.markdown),
    'dart': (label: 'Dart', mode: dart_language.dart),
    'go': (label: 'Go', mode: go_language.go),
    'javascript': (label: 'JavaScript', mode: javascript_language.javascript),
    'typescript': (label: 'TypeScript', mode: typescript_language.typescript),
    'python': (label: 'Python', mode: python_language.python),
    'rust': (label: 'Rust', mode: rust_language.rust),
    'json': (label: 'JSON', mode: json_language.json),
    'yaml': (label: 'YAML', mode: yaml_language.yaml),
    'xml': (label: 'HTML / XML', mode: xml_language.xml),
    'css': (label: 'CSS', mode: css_language.css),
    'sql': (label: 'SQL', mode: sql_language.sql),
  };

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final textStyle = Theme.of(context).textTheme.bodyMedium?.copyWith(
      color: palette.text,
      fontFamily: OpenChatTypography.codeFontFamily,
      fontSize: OpenChatTypography.code,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        DropdownButtonFormField<String>(
          initialValue: languageId,
          decoration: InputDecoration(labelText: languageLabel, isDense: true),
          items: [
            for (final entry in _languages.entries)
              DropdownMenuItem<String>(
                value: entry.key,
                child: Text(entry.value.label),
              ),
          ],
          onChanged: (language) {
            if (language == null) return;
            final definition = _languages[language];
            if (definition == null) return;
            controller.setLanguage(
              definition.mode,
              analyzer: const DefaultLocalAnalyzer(),
            );
            onLanguageChanged(language);
          },
        ),
        const SizedBox(height: 8),
        Align(
          alignment: Alignment.centerLeft,
          child: Text(
            sourceLabel,
            style: Theme.of(context).textTheme.labelMedium,
          ),
        ),
        const SizedBox(height: 6),
        Expanded(
          child: CodeTheme(
            data: CodeThemeData(styles: _syntaxStyles(palette, textStyle)),
            child: CodeField(
              controller: controller,
              expands: true,
              wrap: true,
              textStyle: textStyle,
              onChanged: onChanged,
              gutterStyle: GutterStyle(
                width: 48,
                showErrors: false,
                showFoldingHandles: false,
                textStyle: TextStyle(color: palette.secondaryText),
                background: palette.surface,
              ),
              decoration: BoxDecoration(
                border: Border.all(color: palette.border),
                borderRadius: BorderRadius.circular(OpenChatRadii.control),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Map<String, TextStyle> _syntaxStyles(
    OpenChatPalette palette,
    TextStyle? baseStyle,
  ) => <String, TextStyle>{
    'root': baseStyle ?? TextStyle(color: palette.text),
    '.': TextStyle(color: palette.text),
    'comment': TextStyle(
      color: palette.secondaryText,
      fontStyle: FontStyle.italic,
    ),
    'quote': TextStyle(
      color: palette.secondaryText,
      fontStyle: FontStyle.italic,
    ),
    'keyword': TextStyle(color: palette.accent, fontWeight: FontWeight.w600),
    'selector-tag': TextStyle(
      color: palette.accent,
      fontWeight: FontWeight.w600,
    ),
    'literal': TextStyle(color: palette.info),
    'number': TextStyle(color: palette.warning),
    'string': TextStyle(color: palette.success),
    'regexp': TextStyle(color: palette.info),
    'title': TextStyle(color: palette.info, fontWeight: FontWeight.w600),
    'section': TextStyle(color: palette.accent, fontWeight: FontWeight.w600),
    'built_in': TextStyle(color: palette.info),
    'meta': TextStyle(color: palette.secondaryText),
    'attr': TextStyle(color: palette.warning),
    'addition': TextStyle(color: palette.success),
    'deletion': TextStyle(color: palette.destructive),
    'emphasis': const TextStyle(fontStyle: FontStyle.italic),
    'strong': const TextStyle(fontWeight: FontWeight.w700),
    'bullet': TextStyle(color: palette.accent),
    'symbol': TextStyle(color: palette.info),
    'link': TextStyle(
      color: palette.info,
      decoration: TextDecoration.underline,
    ),
  };
}
