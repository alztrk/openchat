//! Separates in-band model thinking blocks from user-visible streamed text.

const THINK_OPEN_TAG: &str = "<think>";
const THINK_CLOSE_TAG: &str = "</think>";

#[derive(Default)]
pub(super) struct ThinkTagFilter {
    pending: String,
    inside_think: bool,
}

#[derive(Default)]
pub(super) struct ThinkTagOutput {
    pub(super) visible: String,
    pub(super) reasoning: String,
}

impl ThinkTagFilter {
    /// Splits visible and thinking text while buffering partial tags across provider deltas.
    pub(super) fn push(&mut self, text: &str) -> ThinkTagOutput {
        self.pending.push_str(text);
        let mut output = ThinkTagOutput::default();

        loop {
            let tag = if self.inside_think {
                THINK_CLOSE_TAG
            } else {
                THINK_OPEN_TAG
            };
            if let Some(index) = find_ascii_case_insensitive(&self.pending, tag) {
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

            let pending_tag_bytes = longest_suffix_tag_prefix(&self.pending, tag);
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
        if self.inside_think {
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
