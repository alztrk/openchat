import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/safe_markdown.dart';

void main() {
  test('keeps only validated OpenChat citation links', () {
    final html = markdownToSafeHtml(
      '[S1-call1234](openchat-source:S1-call1234) '
      '[P1](openchat-source:P1) '
      '[external](https://example.org) '
      '[invalid](openchat-source:javascript)',
    );

    expect(html, contains('href="openchat-source:S1-call1234"'));
    expect(html, contains('href="openchat-source:P1"'));
    expect(html, isNot(contains('href="https://example.org"')));
    expect(html, isNot(contains('href="openchat-source:javascript"')));
  });
}
