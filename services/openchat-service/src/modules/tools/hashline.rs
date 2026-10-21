//! Hashline-anchored and drift-resilient file replacement engine.
//!
//! Resolves `edit_file` replacements through a deterministic multi-stage pipeline:
//! 1. Exact substring match (`exact`)
//! 2. Line-ending and trailing-whitespace normalized match (`normalized_whitespace`)
//! 3. Hashline-anchored resolution (`hashline_anchor`, e.g. `12:a1b2| code`)
//! 4. Uniform indentation-drift resolution (`indentation_drift`)
//!
//! When replacement cannot be resolved uniquely, returns structured conflict feedback
//! with surrounding `line:hash| text` context lines without modifying the file.

/// Computes a deterministic 4-character lowercase hexadecimal hash tag for a single line.
///
/// Leading and trailing horizontal whitespace is ignored so minor indentation drift
/// does not invalidate a line's structural hash tag.
pub fn compute_hashline_tag(line: &str) -> String {
    let trimmed = line.trim_matches([' ', '\t', '\r', '\n']);
    let bytes = if trimmed.is_empty() {
        line.trim_end_matches(['\r', '\n']).as_bytes()
    } else {
        trimmed.as_bytes()
    };
    let mut hash: u32 = 0x811c_9dc5;
    for &byte in bytes {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    let folded = ((hash >> 16) ^ (hash & 0xffff)) as u16;
    format!("{folded:04x}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedHashline {
    line_number: Option<usize>,
    hash_tag: String,
    text: String,
}

/// Parses a single line that may carry a `12:a1b2| ` or `a1b2| ` hashline prefix.
fn parse_hashline_prefix(line: &str) -> Option<ParsedHashline> {
    let trimmed_start = line.trim_start_matches([' ', '\t']);
    let pipe_idx = trimmed_start.find('|')?;
    if pipe_idx == 0 || pipe_idx > 16 {
        return None;
    }
    let header = &trimmed_start[..pipe_idx];
    let rest = &trimmed_start[pipe_idx + 1..];
    let text = rest.strip_prefix(' ').unwrap_or(rest).to_owned();

    if let Some((line_part, hash_part)) = header.split_once(':') {
        if line_part.is_empty() || !line_part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let line_number = line_part.parse::<usize>().ok()?;
        if hash_part.len() != 4 || !hash_part.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some(ParsedHashline {
            line_number: Some(line_number),
            hash_tag: hash_part.to_ascii_lowercase(),
            text,
        })
    } else if header.len() == 4 && header.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(ParsedHashline {
            line_number: None,
            hash_tag: header.to_ascii_lowercase(),
            text,
        })
    } else {
        None
    }
}

/// Strips accidental `line:hash| ` prefixes from `new_string` if the model echoed them.
fn strip_hashline_prefixes_if_present(new_string: &str, line_ending: &str) -> String {
    let lines: Vec<&str> = new_string.split('\n').collect();
    let non_empty_count = lines
        .iter()
        .filter(|l| !l.trim_end_matches('\r').trim().is_empty())
        .count();
    if non_empty_count == 0 {
        return normalize_line_endings(new_string, line_ending);
    }
    let prefixed_count = lines
        .iter()
        .filter(|l| {
            let clean = l.trim_end_matches('\r');
            !clean.trim().is_empty() && parse_hashline_prefix(clean).is_some()
        })
        .count();

    if prefixed_count * 2 >= non_empty_count && prefixed_count > 0 {
        let stripped: Vec<String> = lines
            .into_iter()
            .map(|raw| {
                let clean = raw.trim_end_matches('\r');
                if let Some(parsed) = parse_hashline_prefix(clean) {
                    parsed.text
                } else {
                    clean.to_owned()
                }
            })
            .collect();
        stripped.join(line_ending)
    } else {
        normalize_line_endings(new_string, line_ending)
    }
}

fn detect_line_ending(content: &str) -> &'static str {
    let crlf_count = content.matches("\r\n").count();
    let lf_only_count = content.matches('\n').count().saturating_sub(crlf_count);
    if crlf_count > lf_only_count {
        "\r\n"
    } else {
        "\n"
    }
}

fn normalize_line_endings(text: &str, target_ending: &str) -> String {
    let normalized = text.replace("\r\n", "\n");
    if target_ending == "\r\n" {
        normalized.replace('\n', "\r\n")
    } else {
        normalized
    }
}

#[derive(Clone, Debug)]
struct FileLineSpan<'a> {
    start_byte: usize,
    end_byte: usize,
    text: &'a str,
    hash_tag: String,
}

fn split_file_lines(content: &str) -> Vec<FileLineSpan<'_>> {
    let mut spans = Vec::new();
    let mut start = 0usize;
    for (idx, ch) in content.char_indices() {
        if ch == '\n' {
            let line_end = if idx > start && content.as_bytes()[idx - 1] == b'\r' {
                idx - 1
            } else {
                idx
            };
            let text = &content[start..line_end];
            spans.push(FileLineSpan {
                start_byte: start,
                end_byte: line_end,
                text,
                hash_tag: compute_hashline_tag(text),
            });
            start = idx + 1;
        }
    }
    if start < content.len() {
        let text = content[start..].trim_end_matches('\r');
        spans.push(FileLineSpan {
            start_byte: start,
            end_byte: content.len(),
            text,
            hash_tag: compute_hashline_tag(text),
        });
    }
    spans
}

fn split_pattern_lines(pattern: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = pattern
        .split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .collect();
    if lines.len() > 1 && lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

/// Applies a multi-stage drift-resilient edit to `content`.
///
/// Returns `Ok((updated_content, match_stage))` on a unique match, or `Err(diagnostic_message)`
/// with structured conflict context when no match or ambiguous matches are found.
pub fn apply_resilient_edit(
    content: &str,
    old_string: &str,
    new_string: &str,
) -> Result<(String, &'static str), String> {
    let line_ending = detect_line_ending(content);
    let old_lines = split_pattern_lines(old_string);

    // Check if the model supplied hashline anchors in old_string.
    let parsed_hashlines: Vec<Option<ParsedHashline>> = old_lines
        .iter()
        .map(|line| parse_hashline_prefix(line))
        .collect();
    let has_hashline_anchors = parsed_hashlines.iter().any(Option::is_some);

    // Stage 1: Exact substring match (when no hashline prefixes are embedded).
    if !has_hashline_anchors {
        let mut exact_matches = content.match_indices(old_string);
        if let Some((start_idx, _)) = exact_matches.next() {
            if exact_matches.next().is_some() {
                let file_lines = split_file_lines(content);
                return Err(build_conflict_diagnostics(
                    &file_lines,
                    &old_lines,
                    true,
                    "The target text is ambiguous and appears multiple times. Provide more unique surrounding context or hashline anchors.",
                ));
            }
            let mut output = String::with_capacity(
                content
                    .len()
                    .saturating_add(new_string.len())
                    .saturating_sub(old_string.len()),
            );
            output.push_str(&content[..start_idx]);
            output.push_str(new_string);
            output.push_str(&content[start_idx + old_string.len()..]);
            return Ok((output, "exact"));
        }
    }

    let file_lines = split_file_lines(content);
    if file_lines.is_empty() || old_lines.is_empty() {
        return Err(
            "The target text to replace was not found in the file (file or target is empty)."
                .to_owned(),
        );
    }

    // Stage 2: Line-ending and trailing-whitespace normalized match.
    if !has_hashline_anchors && old_lines.len() <= file_lines.len() {
        let mut normalized_matches = Vec::new();
        let window_len = old_lines.len();
        for start_line_idx in 0..=(file_lines.len() - window_len) {
            let window = &file_lines[start_line_idx..start_line_idx + window_len];
            let all_match = window
                .iter()
                .zip(old_lines.iter())
                .all(|(file_line, pat_line)| {
                    file_line.text.trim_end_matches([' ', '\t'])
                        == pat_line.trim_end_matches([' ', '\t'])
                });
            if all_match {
                normalized_matches.push(start_line_idx);
            }
        }

        if normalized_matches.len() == 1 {
            let start_idx = normalized_matches[0];
            let end_idx = start_idx + old_lines.len() - 1;
            let clean_new = strip_hashline_prefixes_if_present(new_string, line_ending);
            let clean_new = clean_new.trim_end_matches(['\r', '\n']);
            let replaced = replace_line_span(content, &file_lines, start_idx, end_idx, clean_new);
            return Ok((replaced, "normalized_whitespace"));
        } else if normalized_matches.len() > 1 {
            return Err(build_conflict_diagnostics(
                &file_lines,
                &old_lines,
                true,
                "The target text is ambiguous and appears multiple times after whitespace normalization. Provide more unique surrounding context.",
            ));
        }
    }

    // Stage 3: Hashline-anchored resolution (`12:a1b2| ...`).
    if has_hashline_anchors && old_lines.len() <= file_lines.len() {
        let window_len = old_lines.len();
        let mut anchor_candidates = Vec::new();

        for start_line_idx in 0..=(file_lines.len() - window_len) {
            let window = &file_lines[start_line_idx..start_line_idx + window_len];
            let matches_hashes = window.iter().zip(old_lines.iter().zip(parsed_hashlines.iter())).all(
                |(file_line, (raw_pat, parsed_opt))| {
                    if let Some(parsed) = parsed_opt {
                        // Hash must match, and trimmed content must match if non-empty.
                        let hash_ok = file_line.hash_tag == parsed.hash_tag;
                        let text_ok = parsed.text.trim().is_empty()
                            || file_line.text.trim() == parsed.text.trim();
                        hash_ok && text_ok
                    } else {
                        file_line.text.trim() == raw_pat.trim()
                    }
                },
            );
            if matches_hashes {
                let hinted_line = parsed_hashlines
                    .iter()
                    .find_map(|p| p.as_ref().and_then(|h| h.line_number));
                let distance = hinted_line
                    .map(|line_no| (start_line_idx + 1).abs_diff(line_no))
                    .unwrap_or(0);
                anchor_candidates.push((start_line_idx, distance));
            }
        }

        if !anchor_candidates.is_empty() {
            anchor_candidates.sort_by_key(|&(_, distance)| distance);
            let best_distance = anchor_candidates[0].1;
            let best_matches: Vec<usize> = anchor_candidates
                .iter()
                .filter(|&&(_, d)| d == best_distance)
                .map(|&(idx, _)| idx)
                .collect();

            if best_matches.len() == 1 {
                let start_idx = best_matches[0];
                let end_idx = start_idx + window_len - 1;
                let clean_new = strip_hashline_prefixes_if_present(new_string, line_ending);
                let clean_new = clean_new.trim_end_matches(['\r', '\n']);
                let replaced =
                    replace_line_span(content, &file_lines, start_idx, end_idx, clean_new);
                return Ok((replaced, "hashline_anchor"));
            }
        }
    }

    // Stage 4: Indentation-drift resilient matching.
    if old_lines.len() <= file_lines.len() {
        let pattern_clean_lines: Vec<&str> = old_lines
            .iter()
            .zip(parsed_hashlines.iter())
            .map(|(raw, parsed)| {
                parsed
                    .as_ref()
                    .map(|p| p.text.as_str())
                    .unwrap_or(*raw)
            })
            .collect();

        let has_non_empty = pattern_clean_lines.iter().any(|l| !l.trim().is_empty());
        if has_non_empty {
            let window_len = pattern_clean_lines.len();
            let mut drift_matches = Vec::new();

            for start_line_idx in 0..=(file_lines.len() - window_len) {
                let window = &file_lines[start_line_idx..start_line_idx + window_len];
                let all_trimmed_match = window
                    .iter()
                    .zip(pattern_clean_lines.iter())
                    .all(|(file_line, pat_line)| file_line.text.trim() == pat_line.trim());
                if all_trimmed_match {
                    drift_matches.push(start_line_idx);
                }
            }

            if drift_matches.len() == 1 {
                let start_idx = drift_matches[0];
                let end_idx = start_idx + window_len - 1;
                let clean_new = strip_hashline_prefixes_if_present(new_string, line_ending);
                let clean_new_trimmed = clean_new.trim_end_matches(['\r', '\n']);
                let reindented_new = reindent_replacement(
                    &file_lines[start_idx..=end_idx],
                    &pattern_clean_lines,
                    clean_new_trimmed,
                    line_ending,
                );
                let replaced =
                    replace_line_span(content, &file_lines, start_idx, end_idx, &reindented_new);
                return Ok((replaced, "indentation_drift"));
            } else if drift_matches.len() > 1 {
                return Err(build_conflict_diagnostics(
                    &file_lines,
                    &pattern_clean_lines,
                    true,
                    "The target text is ambiguous and appears multiple times. Provide more unique surrounding context.",
                ));
            }
        }
    }

    Err(build_conflict_diagnostics(
        &file_lines,
        &old_lines,
        false,
        "The target text to replace was not found in the file.",
    ))
}

fn replace_line_span(
    content: &str,
    file_lines: &[FileLineSpan<'_>],
    start_idx: usize,
    end_idx: usize,
    replacement: &str,
) -> String {
    let start_byte = file_lines[start_idx].start_byte;
    let end_byte = file_lines[end_idx].end_byte;
    let mut output = String::with_capacity(
        content
            .len()
            .saturating_add(replacement.len())
            .saturating_sub(end_byte.saturating_sub(start_byte)),
    );
    output.push_str(&content[..start_byte]);
    output.push_str(replacement);
    output.push_str(&content[end_byte..]);
    output
}

fn leading_spaces_count(line: &str) -> usize {
    line.chars()
        .take_while(|&c| c == ' ' || c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn reindent_replacement(
    matched_file_lines: &[FileLineSpan<'_>],
    pattern_lines: &[&str],
    new_text: &str,
    line_ending: &str,
) -> String {
    // Find the first non-empty anchor pair to compute indentation delta.
    let anchor = matched_file_lines
        .iter()
        .zip(pattern_lines.iter())
        .find(|(f, p)| !f.text.trim().is_empty() && !p.trim().is_empty());

    let Some((file_anchor, pat_anchor)) = anchor else {
        return new_text.to_owned();
    };

    let file_indent_str: String = file_anchor
        .text
        .chars()
        .take_while(|&c| c == ' ' || c == '\t')
        .collect();
    let uses_tabs = file_indent_str.contains('\t');
    let file_indent = leading_spaces_count(file_anchor.text);
    let pat_indent = leading_spaces_count(pat_anchor);

    if file_indent == pat_indent {
        return new_text.to_owned();
    }

    let delta = file_indent as isize - pat_indent as isize;
    let new_lines: Vec<String> = new_text
        .split('\n')
        .map(|raw_line| {
            let clean = raw_line.trim_end_matches('\r');
            if clean.trim().is_empty() {
                return String::new();
            }
            let current_indent = leading_spaces_count(clean) as isize;
            let target_indent = (current_indent + delta).max(0) as usize;
            let prefix = if uses_tabs {
                "\t".repeat(target_indent / 4) + &" ".repeat(target_indent % 4)
            } else {
                " ".repeat(target_indent)
            };
            format!("{prefix}{}", clean.trim_start_matches([' ', '\t']))
        })
        .collect();

    new_lines.join(line_ending)
}

fn build_conflict_diagnostics(
    file_lines: &[FileLineSpan<'_>],
    pattern_lines: &[&str],
    is_ambiguous: bool,
    base_message: &str,
) -> String {
    let mut best_line_idx = None;
    let mut best_score = 0usize;

    let query_probes: Vec<&str> = pattern_lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .take(8)
        .collect();

    if !query_probes.is_empty() && !file_lines.is_empty() {
        for idx in 0..file_lines.len() {
            let mut score = 0usize;
            for (offset, probe) in query_probes.iter().enumerate() {
                let Some(candidate) = file_lines.get(idx + offset) else {
                    break;
                };
                let cand_trim = candidate.text.trim();
                if cand_trim == *probe {
                    score += 4;
                } else if !cand_trim.is_empty()
                    && (cand_trim.contains(probe) || probe.contains(cand_trim))
                {
                    score += 2;
                }
            }
            if score > best_score {
                best_score = score;
                best_line_idx = Some(idx);
            }
        }
    }

    let Some(center_idx) = best_line_idx else {
        return base_message.to_owned();
    };

    let context_start = center_idx.saturating_sub(2);
    let context_end = (center_idx + pattern_lines.len() + 2).min(file_lines.len());
    let mut snippet = String::new();
    for (idx, span) in file_lines[context_start..context_end].iter().enumerate() {
        let line_no = context_start + idx + 1;
        if !snippet.is_empty() {
            snippet.push('\n');
        }
        snippet.push_str(&format!("{line_no}:{}| {}", span.hash_tag, span.text));
    }

    let hint = if is_ambiguous {
        "Nearest matching context with hashline anchors"
    } else {
        "Closest file context around line mismatch (use exact text or `line:hash| text` anchors)"
    };
    format!("{base_message}\n\n{hint}:\n{snippet}")
}

#[cfg(test)]
mod tests {
    use super::{apply_resilient_edit, compute_hashline_tag};

    #[test]
    fn exact_and_crlf_normalized_replacements_preserve_file_line_endings() {
        let content = "fn first() {\r\n    println!(\"one\");\r\n}\r\n";
        let (updated, stage) = apply_resilient_edit(
            content,
            "fn first() {\n    println!(\"one\");\n}",
            "fn first() {\n    println!(\"two\");\n}",
        )
        .expect("normalized CRLF edit should succeed");
        assert_eq!(stage, "normalized_whitespace");
        assert_eq!(updated, "fn first() {\r\n    println!(\"two\");\r\n}\r\n");
    }

    #[test]
    fn indentation_drift_is_resolved_and_reindented_cleanly() {
        let content = "impl Service {\n    fn compute(&self) -> i32 {\n        let value = 41;\n        value + 1\n    }\n}\n";
        // Model sends 0-space outer indent instead of 4-space outer indent.
        let (updated, stage) = apply_resilient_edit(
            content,
            "fn compute(&self) -> i32 {\n    let value = 41;\n    value + 1\n}",
            "fn compute(&self) -> i32 {\n    let value = 99;\n    value + 1\n}",
        )
        .expect("indentation drift edit should succeed");
        assert_eq!(stage, "indentation_drift");
        assert_eq!(
            updated,
            "impl Service {\n    fn compute(&self) -> i32 {\n        let value = 99;\n        value + 1\n    }\n}\n"
        );
    }

    #[test]
    fn hashline_anchors_resolve_target_lines_and_strip_echoed_prefixes() {
        let line2 = "    let alpha = 10;";
        let line3 = "    let beta = 20;";
        let h2 = compute_hashline_tag(line2);
        let h3 = compute_hashline_tag(line3);
        let content = format!("fn main() {{\n{line2}\n{line3}\n}}\n");

        let old_with_hashes = format!("2:{h2}| {line2}\n3:{h3}| {line3}");
        let new_with_hashes = format!("2:{h2}|     let alpha = 100;\n3:{h3}|     let beta = 200;");

        let (updated, stage) = apply_resilient_edit(&content, &old_with_hashes, &new_with_hashes)
            .expect("hashline anchored edit should succeed");
        assert_eq!(stage, "hashline_anchor");
        assert_eq!(
            updated,
            "fn main() {\n    let alpha = 100;\n    let beta = 200;\n}\n"
        );
    }

    #[test]
    fn failed_match_returns_structured_hashline_conflict_diagnostics() {
        let content = "fn calculate() {\n    let total = price * quantity;\n    return total;\n}\n";
        let err = apply_resilient_edit(
            content,
            "fn calculate() {\n    let total = wrong_variable;\n    return total;\n}",
            "fn calculate() {}",
        )
        .expect_err("mismatched line should fail with diagnostics");

        assert!(err.contains("The target text to replace was not found in the file."));
        assert!(err.contains("Closest file context"));
        assert!(err.contains("let total = price * quantity;"));
    }
}
