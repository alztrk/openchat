import 'package:html/parser.dart' as html_parser;
import 'package:markdown/markdown.dart' as markdown;

String markdownToSafeHtml(String source) {
  final rendered = markdown.markdownToHtml(
    source,
    extensionSet: markdown.ExtensionSet.gitHubFlavored,
    encodeHtml: false,
  );
  return sanitizeMarkdownHtml(rendered);
}

String sanitizeMarkdownHtml(String source) {
  final fragment = html_parser.parseFragment(source);
  const blockedTags = <String>{
    'script',
    'style',
    'iframe',
    'object',
    'embed',
    'form',
    'input',
    'textarea',
    'select',
    'option',
    'button',
    'img',
    'video',
    'audio',
    'source',
    'link',
    'meta',
    'base',
    'svg',
    'canvas',
  };
  for (final element in fragment.querySelectorAll('*').toList()) {
    if (blockedTags.contains(element.localName)) {
      element.remove();
      continue;
    }
    if (element.localName == 'a' &&
        _isOpenChatCitationUri(element.attributes['href'])) {
      element.attributes.removeWhere(
        (name, _) => !const {'class', 'href'}.contains(name),
      );
      continue;
    }
    element.attributes.removeWhere(
      (name, _) =>
          !const {'class', 'colspan', 'rowspan', 'start'}.contains(name),
    );
  }
  return fragment.outerHtml;
}

bool _isOpenChatCitationUri(String? value) {
  final uri = value == null ? null : Uri.tryParse(value);
  return uri?.scheme == 'openchat-source' &&
      RegExp(r'^[PSU]\d+(?:-[A-Za-z0-9]+)?$').hasMatch(uri!.path);
}
