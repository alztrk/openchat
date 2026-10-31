//! Separates in-band model thinking blocks from user-visible streamed text.

const THINK_OPEN_TAG: &str = "<think>";
const THINK_CLOSE_TAG: &str = "</think>";
const THINKING_OPEN_TAG: &str = "<thinking>";
const THINKING_CLOSE_TAG: &str = "</thinking>";

#[derive(Default)]
pub(super) struct ThinkTagFilter {
    pending: String,
    inside_think: bool,
    closing_tag_only_candidate: bool,
}

#[derive(Default)]
pub(super) struct ThinkTagOutput {
    pub(super) visible: String,
    pub(super) reasoning: String,
}

impl ThinkTagFilter {
    pub(super) fn new() -> Self {
        Self {
            pending: String::new(),
            inside_think: false,
            closing_tag_only_candidate: true,
        }
    }

    /// Splits visible and thinking text while buffering partial tags across provider deltas.
    pub(super) fn push(&mut self, text: &str) -> ThinkTagOutput {
        self.pending.push_str(text);
        let mut output = ThinkTagOutput::default();

        loop {
            let open_tags = [THINK_OPEN_TAG, THINKING_OPEN_TAG];
            let close_tags = [THINK_CLOSE_TAG, THINKING_CLOSE_TAG];
            if !self.inside_think && self.closing_tag_only_candidate {
                let opening = open_tags
                    .iter()
                    .filter_map(|tag| {
                        find_ascii_case_insensitive(&self.pending, tag).map(|i| (i, *tag))
                    })
                    .min_by_key(|(index, _)| *index);
                let closing = close_tags
                    .iter()
                    .filter_map(|tag| {
                        find_ascii_case_insensitive(&self.pending, tag).map(|i| (i, *tag))
                    })
                    .min_by_key(|(index, _)| *index);

                match (opening, closing) {
                    (Some((open_index, _)), Some((close_index, close_tag)))
                        if close_index < open_index =>
                    {
                        let reasoning = self.pending.drain(..close_index).collect::<String>();
                        output.reasoning.push_str(&reasoning);
                        self.pending.drain(..close_tag.len());
                        self.closing_tag_only_candidate = false;
                        continue;
                    }
                    (Some((open_index, open_tag)), _) => {
                        let visible = self.pending.drain(..open_index).collect::<String>();
                        output.visible.push_str(&visible);
                        self.pending.drain(..open_tag.len());
                        self.inside_think = true;
                        self.closing_tag_only_candidate = false;
                        continue;
                    }
                    (_, Some((close_index, close_tag))) => {
                        let reasoning = self.pending.drain(..close_index).collect::<String>();
                        output.reasoning.push_str(&reasoning);
                        self.pending.drain(..close_tag.len());
                        self.closing_tag_only_candidate = false;
                        continue;
                    }
                    (None, None) => break,
                }
            }

            let tags = if self.inside_think {
                &close_tags
            } else {
                &open_tags
            };
            let found = tags
                .iter()
                .filter_map(|tag| {
                    find_ascii_case_insensitive(&self.pending, tag).map(|i| (i, *tag))
                })
                .min_by_key(|(index, _)| *index);
            if let Some((index, tag)) = found {
                let segment = self.pending.drain(..index).collect::<String>();
                if self.inside_think {
                    output.reasoning.push_str(&segment);
                } else {
                    output.visible.push_str(&segment);
                }
                self.pending.drain(..tag.len());
                self.inside_think = !self.inside_think;
                continue;
            }

            let pending_tag_bytes = tags
                .iter()
                .map(|tag| longest_suffix_tag_prefix(&self.pending, tag))
                .max()
                .unwrap_or_default();
            let segment_bytes = self.pending.len().saturating_sub(pending_tag_bytes);
            let segment = self.pending.drain(..segment_bytes).collect::<String>();
            if self.inside_think {
                output.reasoning.push_str(&segment);
            } else {
                output.visible.push_str(&segment);
            }
            break;
        }

        output
    }

    /// Releases incomplete trailing text into the group indicated by the current state.
    pub(super) fn finish(&mut self) -> ThinkTagOutput {
        let trailing = std::mem::take(&mut self.pending);
        if self.inside_think && !self.closing_tag_only_candidate {
            ThinkTagOutput {
                visible: String::new(),
                reasoning: trailing,
            }
        } else {
            ThinkTagOutput {
                visible: trailing,
                reasoning: String::new(),
            }
        }
    }
}

fn find_ascii_case_insensitive(value: &str, tag: &str) -> Option<usize> {
    value
        .as_bytes()
        .windows(tag.len())
        .position(|window| window.eq_ignore_ascii_case(tag.as_bytes()))
}

fn longest_suffix_tag_prefix(value: &str, tag: &str) -> usize {
    let value = value.as_bytes();
    let tag = tag.as_bytes();
    for length in (1..tag.len()).rev() {
        if value.len() >= length
            && value[value.len() - length..].eq_ignore_ascii_case(&tag[..length])
        {
            return length;
        }
    }
    0
}
