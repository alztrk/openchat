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
    element.attributes.removeWhere(
      (name, _) =>
          !const {'class', 'colspan', 'rowspan', 'start'}.contains(name),
    );
  }
  return fragment.outerHtml;
}
