import 'package:flutter/services.dart';

enum ComposerShortcutAction { sendMessage, insertNewLine, stopResponse }

class KeyboardShortcutBinding {
  const KeyboardShortcutBinding({
    required this.keyId,
    this.control = false,
    this.alt = false,
    this.shift = false,
    this.meta = false,
  });

  final int keyId;
  final bool control;
  final bool alt;
  final bool shift;
  final bool meta;

  bool hasSameKeys(KeyboardShortcutBinding other) =>
      keyId == other.keyId &&
      control == other.control &&
      alt == other.alt &&
      shift == other.shift &&
      meta == other.meta;

  bool matches(KeyEvent event, HardwareKeyboard keyboard) =>
      event.logicalKey.keyId == keyId &&
      keyboard.isControlPressed == control &&
      keyboard.isAltPressed == alt &&
      keyboard.isShiftPressed == shift &&
      keyboard.isMetaPressed == meta;

  String get displayLabel {
    final key = LogicalKeyboardKey.findKeyByKeyId(keyId);
    if (key == null) return '';
    return <String>[
      if (control) 'Ctrl',
      if (alt) 'Alt',
      if (shift) 'Shift',
      if (meta) 'Meta',
      key.keyLabel,
    ].join(' + ');
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'keyId': keyId,
    'control': control,
    'alt': alt,
    'shift': shift,
    'meta': meta,
  };

  static KeyboardShortcutBinding fromJson(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('A keyboard shortcut was invalid.');
    }
    final keyId = value['keyId'];
    final control = value['control'];
    final alt = value['alt'];
    final shift = value['shift'];
    final meta = value['meta'];
    if (keyId is! int ||
        LogicalKeyboardKey.findKeyByKeyId(keyId) == null ||
        control is! bool ||
        alt is! bool ||
        shift is! bool ||
        meta is! bool) {
      throw const FormatException('A keyboard shortcut was invalid.');
    }
    return KeyboardShortcutBinding(
      keyId: keyId,
      control: control,
      alt: alt,
      shift: shift,
      meta: meta,
    );
  }

  static KeyboardShortcutBinding fromKeyEvent(
    KeyEvent event,
    HardwareKeyboard keyboard,
  ) => KeyboardShortcutBinding(
    keyId: event.logicalKey.keyId,
    control: keyboard.isControlPressed,
    alt: keyboard.isAltPressed,
    shift: keyboard.isShiftPressed,
    meta: keyboard.isMetaPressed,
  );
}

class KeyboardShortcutBindings {
  const KeyboardShortcutBindings({
    required this.sendMessage,
    required this.insertNewLine,
    required this.stopResponse,
  });

  static const defaults = KeyboardShortcutBindings(
    sendMessage: KeyboardShortcutBinding(keyId: 0x000d),
    insertNewLine: KeyboardShortcutBinding(keyId: 0x000d, shift: true),
    stopResponse: KeyboardShortcutBinding(keyId: 0x001b),
  );

  final KeyboardShortcutBinding sendMessage;
  final KeyboardShortcutBinding insertNewLine;
  final KeyboardShortcutBinding stopResponse;

  KeyboardShortcutBinding forAction(ComposerShortcutAction action) =>
      switch (action) {
        ComposerShortcutAction.sendMessage => sendMessage,
        ComposerShortcutAction.insertNewLine => insertNewLine,
        ComposerShortcutAction.stopResponse => stopResponse,
      };

  KeyboardShortcutBindings withBinding(
    ComposerShortcutAction action,
    KeyboardShortcutBinding binding,
  ) => switch (action) {
    ComposerShortcutAction.sendMessage => KeyboardShortcutBindings(
      sendMessage: binding,
      insertNewLine: insertNewLine,
      stopResponse: stopResponse,
    ),
    ComposerShortcutAction.insertNewLine => KeyboardShortcutBindings(
      sendMessage: sendMessage,
      insertNewLine: binding,
      stopResponse: stopResponse,
    ),
    ComposerShortcutAction.stopResponse => KeyboardShortcutBindings(
      sendMessage: sendMessage,
      insertNewLine: insertNewLine,
      stopResponse: binding,
    ),
  };

  bool conflicts(
    ComposerShortcutAction action,
    KeyboardShortcutBinding binding,
  ) {
    return ComposerShortcutAction.values.any(
      (other) => other != action && forAction(other).hasSameKeys(binding),
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'sendMessage': sendMessage.toJson(),
    'insertNewLine': insertNewLine.toJson(),
    'stopResponse': stopResponse.toJson(),
  };

  static KeyboardShortcutBindings fromJson(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('Keyboard shortcuts were invalid.');
    }
    final bindings = KeyboardShortcutBindings(
      sendMessage: KeyboardShortcutBinding.fromJson(value['sendMessage']),
      insertNewLine: KeyboardShortcutBinding.fromJson(value['insertNewLine']),
      stopResponse: KeyboardShortcutBinding.fromJson(value['stopResponse']),
    );
    final actions = ComposerShortcutAction.values;
    if (actions.any(
      (action) => bindings.conflicts(action, bindings.forAction(action)),
    )) {
      throw const FormatException('Keyboard shortcuts conflict.');
    }
    return bindings;
  }
}
