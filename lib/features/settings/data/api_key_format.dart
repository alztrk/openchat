abstract final class ApiKeyFormat {
  static const maximumLength = 4096;

  static final _printableKey = RegExp(r'^[\x21-\x7E]+$');
  static final _documentedPrefixes = <String, RegExp>{
    'chatgpt_api': RegExp(r'^sk-[\x21-\x7E]+$'),
    'gemini': RegExp(r'^(?:AIza|AQ\.)[\x21-\x7E]+$'),
    'groq': RegExp(r'^gsk_[\x21-\x7E]+$'),
    'openrouter': RegExp(r'^sk-or-v1-[\x21-\x7E]+$'),
  };

  static bool isValid(String providerId, String key) {
    if (key.length > maximumLength || !_printableKey.hasMatch(key)) {
      return false;
    }
    return _documentedPrefixes[providerId]?.hasMatch(key) ?? true;
  }
}
