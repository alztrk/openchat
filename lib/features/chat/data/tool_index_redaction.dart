import 'package:sqlite3/sqlite3.dart' as sqlite;

const int _maxIndexTextCodeUnits = 8 * 1024 * 1024;
const String _oversizedIndexText =
    '[tool activity omitted: searchable archive limit exceeded]';

const List<String> _sensitiveKeys = <String>[
  'aws_secret_access_key',
  'cerebras_api_key',
  'huggingface_api_key',
  'openrouter_api_key',
  'github_access_token',
  'google_api_key',
  'openai_api_key',
  'mistral_api_key',
  'gemini_api_key',
  'replicate_api_token',
  'groq_api_key',
  'secret_access_key',
  'bearer_token',
  'proxy-authorization',
  'authorization',
  'client_secret',
  'clientsecret',
  'auth_token',
  'api_token',
  'refresh_token',
  'access_token',
  'github_token',
  'private_key',
  'password',
  'passwd',
  'api_key',
  'api-key',
  'apikey',
  'id_token',
  'x-api-key',
];

const List<(String, int)> _tokenPrefixes = <(String, int)>[
  ('github_pat_', 28),
  ('sk-or-v1-', 28),
  ('sk-ant-', 22),
  ('sk_live_', 28),
  ('sk_test_', 28),
  ('rk_live_', 28),
  ('rk_test_', 28),
  ('ghp_', 24),
  ('gho_', 24),
  ('ghu_', 24),
  ('ghs_', 24),
  ('ghr_', 24),
  ('AIza', 32),
  ('gsk_', 24),
  ('glpat-', 26),
  ('pypi-', 32),
  ('csk-', 24),
  ('xoxb-', 26),
  ('xoxp-', 26),
  ('xoxr-', 26),
  ('xoxs-', 26),
  ('xapp-', 28),
  ('npm_', 24),
  ('whsec_', 24),
  ('ya29.', 32),
  ('AKIA', 20),
  ('ASIA', 20),
  ('hf_', 24),
  ('xai-', 24),
  ('r8_', 24),
  ('sk-', 28),
];

/// Redacts common credential fields and token formats from derived tool indexes.
String redactToolIndexText(String input) {
  if (input.length > _maxIndexTextCodeUnits) return _oversizedIndexText;

  final ranges = _sensitiveValueRanges(input);
  ranges.addAll(_credentialTokenRanges(input, ranges));
  ranges.sort((a, b) {
    final startOrder = a.$1.compareTo(b.$1);
    return startOrder == 0 ? a.$2.compareTo(b.$2) : startOrder;
  });

  final output = StringBuffer();
  var cursor = 0;
  for (final range in ranges) {
    if (range.$1 < cursor) {
      if (range.$2 > cursor) cursor = range.$2;
      continue;
    }
    output
      ..write(input.substring(cursor, range.$1))
      ..write('[REDACTED]');
    cursor = range.$2;
  }
  output.write(input.substring(cursor));
  return output.toString();
}

Object? redactToolIndexSqlFunction(List<Object?> arguments) {
  final value = arguments.single;
  return value is String ? redactToolIndexText(value) : value;
}

void registerToolIndexRedaction(sqlite.Database database) {
  database.createFunction(
    functionName: 'openchat_redact_credentials',
    argumentCount: const sqlite.AllowedArgumentCount(1),
    deterministic: true,
    directOnly: false,
    function: redactToolIndexSqlFunction,
  );
}

List<(int, int)> _sensitiveValueRanges(String input) {
  final ranges = <(int, int)>[];
  var index = 0;
  while (index < input.length) {
    String? matchedKey;
    for (final key in _sensitiveKeys) {
      if (_keyMatchesAt(input, index, key)) {
        matchedKey = key;
        break;
      }
    }
    if (matchedKey == null) {
      index++;
      continue;
    }

    var cursor = index + matchedKey.length;
    if (cursor < input.length && _isQuote(input.codeUnitAt(cursor))) cursor++;
    cursor = _skipWhitespace(input, cursor);
    if (cursor >= input.length || !_isAssignment(input.codeUnitAt(cursor))) {
      index++;
      continue;
    }
    cursor = _skipWhitespace(input, cursor + 1);
    if (cursor >= input.length) {
      index += matchedKey.length;
      continue;
    }

    final first = input.codeUnitAt(cursor);
    final quoted = _isQuote(first);
    final start = quoted ? cursor + 1 : cursor;
    final end = quoted
        ? _quotedValueEnd(input, start, first)
        : _isAuthorizationKey(matchedKey)
        ? _authorizationValueEnd(input, start)
        : _unquotedValueEnd(input, start);
    if (end > start) {
      ranges.add((start, end));
      index = end;
    } else {
      index += matchedKey.length;
    }
  }
  return ranges;
}

List<(int, int)> _credentialTokenRanges(
  String input,
  List<(int, int)> existing,
) {
  final ranges = <(int, int)>[];
  var index = 0;
  var existingIndex = 0;
  while (index < input.length) {
    while (existingIndex < existing.length &&
        existing[existingIndex].$2 <= index) {
      existingIndex++;
    }
    if (existingIndex < existing.length &&
        existing[existingIndex].$1 <= index &&
        index < existing[existingIndex].$2) {
      index = existing[existingIndex].$2;
      continue;
    }

    (String, int)? match;
    for (final prefix in _tokenPrefixes) {
      if (_startsAsciiCaseInsensitive(input, index, prefix.$1) &&
          (index == 0 || !_isAsciiWord(input.codeUnitAt(index - 1)))) {
        match = prefix;
        break;
      }
    }
    if (match == null) {
      index++;
      continue;
    }

    var end = index;
    while (end < input.length && _isTokenCodeUnit(input.codeUnitAt(end))) {
      end++;
    }
    while (end > index &&
        _isTrailingTokenSeparator(input.codeUnitAt(end - 1))) {
      end--;
    }
    if (end - index >= match.$2) {
      ranges.add((index, end));
      index = end;
    } else {
      index++;
    }
  }
  return ranges;
}

bool _keyMatchesAt(String input, int index, String key) =>
    _startsAsciiCaseInsensitive(input, index, key) &&
    (index == 0 || !_isAsciiWord(input.codeUnitAt(index - 1))) &&
    (index + key.length == input.length ||
        !_isAsciiWord(input.codeUnitAt(index + key.length)));

bool _startsAsciiCaseInsensitive(String input, int index, String pattern) {
  if (index + pattern.length > input.length) return false;
  for (var offset = 0; offset < pattern.length; offset++) {
    if (_asciiLower(input.codeUnitAt(index + offset)) !=
        _asciiLower(pattern.codeUnitAt(offset))) {
      return false;
    }
  }
  return true;
}

int _asciiLower(int codeUnit) =>
    codeUnit >= 0x41 && codeUnit <= 0x5a ? codeUnit + 0x20 : codeUnit;

int _skipWhitespace(String input, int index) {
  while (index < input.length && _isWhitespace(input.codeUnitAt(index))) {
    index++;
  }
  return index;
}

int _quotedValueEnd(String input, int index, int quote) {
  while (index < input.length) {
    final codeUnit = input.codeUnitAt(index);
    if (codeUnit == 0x5c) {
      index += 2;
      if (index > input.length) index = input.length;
      continue;
    }
    if (codeUnit == quote) break;
    index++;
  }
  return index;
}

int _unquotedValueEnd(String input, int index) {
  while (index < input.length) {
    final codeUnit = input.codeUnitAt(index);
    if (_isWhitespace(codeUnit) || _isValueDelimiter(codeUnit)) break;
    index++;
  }
  return index;
}

int _authorizationValueEnd(String input, int index) {
  while (index < input.length &&
      !_isAuthorizationDelimiter(input.codeUnitAt(index))) {
    index++;
  }
  while (index > 0 && _isWhitespace(input.codeUnitAt(index - 1))) {
    index--;
  }
  return index;
}

bool _isAuthorizationKey(String key) =>
    key.toLowerCase() == 'authorization' ||
    key.toLowerCase() == 'proxy-authorization';

bool _isWhitespace(int codeUnit) =>
    codeUnit == 0x20 || (codeUnit >= 0x09 && codeUnit <= 0x0d);

bool _isQuote(int codeUnit) => codeUnit == 0x22 || codeUnit == 0x27;

bool _isAssignment(int codeUnit) => codeUnit == 0x3a || codeUnit == 0x3d;

bool _isValueDelimiter(int codeUnit) =>
    codeUnit == 0x2c ||
    codeUnit == 0x3b ||
    codeUnit == 0x7d ||
    codeUnit == 0x5d ||
    codeUnit == 0x26 ||
    codeUnit == 0x22 ||
    codeUnit == 0x27;

bool _isAuthorizationDelimiter(int codeUnit) =>
    codeUnit == 0x0a ||
    codeUnit == 0x0d ||
    codeUnit == 0x2c ||
    codeUnit == 0x3b ||
    codeUnit == 0x7d ||
    codeUnit == 0x5d ||
    codeUnit == 0x22 ||
    codeUnit == 0x27;

bool _isAsciiWord(int codeUnit) =>
    (codeUnit >= 0x30 && codeUnit <= 0x39) ||
    (codeUnit >= 0x41 && codeUnit <= 0x5a) ||
    (codeUnit >= 0x61 && codeUnit <= 0x7a) ||
    codeUnit == 0x5f;

bool _isTokenCodeUnit(int codeUnit) =>
    _isAsciiWord(codeUnit) || codeUnit == 0x2d || codeUnit == 0x2e;

bool _isTrailingTokenSeparator(int codeUnit) =>
    codeUnit == 0x2d || codeUnit == 0x5f || codeUnit == 0x2e;
