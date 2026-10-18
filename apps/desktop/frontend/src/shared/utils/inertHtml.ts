/** Parses untrusted markup in an inert template; callers must still render only allowlisted nodes. */
export function parseInertHtmlFragment(markup: string): DocumentFragment | null {
  if (typeof document === 'undefined') return null;

  const template = document.createElement('template');
  template.innerHTML = markup;
  return template.content;
}
