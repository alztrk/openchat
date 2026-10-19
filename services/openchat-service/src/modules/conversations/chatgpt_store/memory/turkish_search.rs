//! Turkish character folding and search normalization for FTS5 queries.
//!
//! Provides bidirectional normalization for Turkish-specific characters
//! (such as dotted 'i' / dotless 'ı' and diacritics) so SQLite FTS5 index
//! queries reliably match both localized and ASCII-folded inputs.

use std::collections::HashSet;

/// Converts a character to its ASCII folded equivalent if it is a Turkish-specific letter.
#[inline]
pub fn turkish_fold_char(character: char) -> char {
    match character {
        'ı' | 'I' => 'i',
        'İ' | 'i' => 'i',
        'ğ' | 'Ğ' => 'g',
        'ü' | 'Ü' => 'u',
        'ş' | 'Ş' => 's',
        'ö' | 'Ö' => 'o',
        'ç' | 'Ç' => 'c',
        other => other,
    }
}

/// Converts a string to Turkish lowercase, respecting dotted 'İ' -> 'i' and dotless 'I' -> 'ı'.
pub fn turkish_to_lowercase(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            'I' => result.push('ı'),
            'İ' => result.push('i'),
            other => {
                for lower in other.to_lowercase() {
                    result.push(lower);
                }
            }
        }
    }
    result
}

/// Folds a Turkish string into an ASCII-equivalent lowercase representation.
pub fn turkish_to_ascii(text: &str) -> String {
    let lower = turkish_to_lowercase(text);
    lower.chars().map(turkish_fold_char).collect()
}

/// Returns whether the string contains any Turkish-specific diacritic or case-sensitive characters.
#[allow(dead_code)]
pub fn contains_turkish_chars(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character,
            'ı' | 'I' | 'İ' | 'ğ' | 'Ğ' | 'ü' | 'Ü' | 'ş' | 'Ş' | 'ö' | 'Ö' | 'ç' | 'Ç'
        )
    })
}

/// Expands a search term into its Turkish and ASCII-folded variants for FTS5 query expansion.
///
/// Guaranteed to:
/// 1. Keep the original lowercase term as the first item.
/// 2. If the term contains Turkish letters ('ı', 'İ', 'ğ', 'ü', 'ş', 'ö', 'ç'), include the ASCII folded equivalent.
/// 3. If the term contains 'i', produce an 'ı' variant for cross-matching dotless forms.
/// 4. If the term contains 'ı', produce an 'i' variant.
/// 5. Deduplicate and return distinct search tokens.
pub fn expand_turkish_search_terms(raw_term: &str) -> Vec<String> {
    let mut variants = Vec::with_capacity(4);
    let mut seen = HashSet::with_capacity(4);

    let turkish_lower = turkish_to_lowercase(raw_term);
    if !turkish_lower.is_empty() && seen.insert(turkish_lower.clone()) {
        variants.push(turkish_lower.clone());
    }

    let ascii_folded = turkish_to_ascii(&turkish_lower);
    if !ascii_folded.is_empty() && seen.insert(ascii_folded.clone()) {
        variants.push(ascii_folded);
    }

    // Bidirectional dotless/dotted swap: 'i' <-> 'ı'
    if turkish_lower.contains('i') {
        let dotless_variant: String = turkish_lower
            .chars()
            .map(|character| if character == 'i' { 'ı' } else { character })
            .collect();
        if seen.insert(dotless_variant.clone()) {
            variants.push(dotless_variant);
        }
    } else if turkish_lower.contains('ı') {
        let dotted_variant: String = turkish_lower
            .chars()
            .map(|character| if character == 'ı' { 'i' } else { character })
            .collect();
        if seen.insert(dotted_variant.clone()) {
            variants.push(dotted_variant);
        }
    }

    variants
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turkish_to_lowercase() {
        assert_eq!(turkish_to_lowercase("IŞIK"), "ışık");
        assert_eq!(turkish_to_lowercase("İSTANBUL"), "istanbul");
        assert_eq!(turkish_to_lowercase("ÇALIŞMA"), "çalışma");
        assert_eq!(turkish_to_lowercase("ÖĞRENCİ"), "öğrenci");
        assert_eq!(turkish_to_lowercase("HELLO WORLD"), "hello world");
    }

    #[test]
    fn test_turkish_to_ascii() {
        assert_eq!(turkish_to_ascii("ışık"), "isik");
        assert_eq!(turkish_to_ascii("IŞIK"), "isik");
        assert_eq!(turkish_to_ascii("İstanbul"), "istanbul");
        assert_eq!(turkish_to_ascii("çalışma"), "calisma");
        assert_eq!(turkish_to_ascii("öğrenci"), "ogrenci");
    }

    #[test]
    fn test_contains_turkish_chars() {
        assert!(contains_turkish_chars("ışık"));
        assert!(contains_turkish_chars("İstanbul"));
        assert!(contains_turkish_chars("GÜN"));
        assert!(!contains_turkish_chars("hello"));
        assert!(!contains_turkish_chars("12345"));
    }

    #[test]
    fn test_expand_turkish_search_terms() {
        let isik_variants = expand_turkish_search_terms("ışık");
        assert_eq!(isik_variants[0], "ışık");
        assert!(isik_variants.contains(&"isik".to_string()));

        let ascii_isik_variants = expand_turkish_search_terms("isik");
        assert_eq!(ascii_isik_variants[0], "isik");
        assert!(ascii_isik_variants.contains(&"ısık".to_string()));

        let istanbul_variants = expand_turkish_search_terms("İSTANBUL");
        assert_eq!(istanbul_variants[0], "istanbul");
        assert!(istanbul_variants.contains(&"ıstanbul".to_string()));

        let plain_variants = expand_turkish_search_terms("plan");
        assert_eq!(plain_variants, vec!["plan"]);
    }
}
