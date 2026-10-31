export interface InBandReasoning {
  visibleContent: string;
  reasoningContent: string;
}

const reasoningTagPattern = /<\/?think(?:ing)?\s*>/gi;

/** Separates reasoning tags that older or non-streaming provider responses may leave in content. */
export function separateInBandReasoning(content: string): InBandReasoning {
  const visible: string[] = [];
  const reasoning: string[] = [];
  let cursor = 0;
  let insideReasoning = false;
  let sawOpeningTag = false;

  for (const match of content.matchAll(reasoningTagPattern)) {
    const index = match.index;
    const tag = match[0];
    const isClosingTag = tag.startsWith('</');
    const segment = content.slice(cursor, index);

    if (insideReasoning) {
      reasoning.push(segment);
    } else if (isClosingTag && !sawOpeningTag && visible.length === 0) {
      reasoning.push(segment);
    } else {
      visible.push(segment);
    }

    if (isClosingTag) {
      insideReasoning = false;
    } else {
      insideReasoning = true;
      sawOpeningTag = true;
    }
    cursor = index + tag.length;
  }

  const trailing = content.slice(cursor);
  if (insideReasoning) reasoning.push(trailing);
  else visible.push(trailing);

  return {
    visibleContent: visible.join(''),
    reasoningContent: reasoning.join(''),
  };
}
