//! Removes in-band model thinking blocks from user-visible streamed text.

const THINK_OPEN_TAG: &str = "<think>";
const THINK_CLOSE_TAG: &str = "</think>";

#[derive(Default)]
pub(super) struct ThinkTagFilter {
    pending: String,
    inside_think: bool,
}

impl ThinkTagFilter {
    /// Returns visible text while buffering partial tags across provider deltas.
    pub(super) fn push(&mut self, text: &str) -> String {
        self.pending.push_str(text);
        let mut visible = String::with_capacity(self.pending.len());

        loop {
            let tag = if self.inside_think {
                THINK_CLOSE_TAG
            } else {
                THINK_OPEN_TAG
            };
            if let Some(index) = find_ascii_case_insensitive(&self.pending, tag) {
                if !self.inside_think {
                    visible.push_str(&self.pending[..index]);
                }
                self.pending.drain(..index + tag.len());
                self.inside_think = !self.inside_think;
                continue;
            }

            let pending_tag_bytes = longest_suffix_tag_prefix(&self.pending, tag);
            let visible_bytes = self.pending.len().saturating_sub(pending_tag_bytes);
            if !self.inside_think && visible_bytes > 0 {
                visible.push_str(&self.pending[..visible_bytes]);
            }
            self.pending.drain(..visible_bytes);
            break;
        }

        visible
    }

    /// Releases a trailing partial opening tag, but never releases an unfinished thinking block.
    pub(super) fn finish(&mut self) -> String {
        if self.inside_think {
            self.pending.clear();
            String::new()
        } else {
            std::mem::take(&mut self.pending)
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
