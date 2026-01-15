//! Shared utility functions for safe-coder
//!
//! Contains common helpers used across the codebase.

/// Safely truncate a string to max_chars characters (not bytes)
/// Avoids panic on multi-byte UTF-8 characters
///
/// # Examples
/// ```
/// use safe_coder::utils::truncate_str;
///
/// // ASCII strings work as expected
/// assert_eq!(truncate_str("hello world", 5), "hello");
///
/// // Multi-byte UTF-8 characters are handled correctly
/// let emoji = "hello 🌍 world";
/// let truncated = truncate_str(emoji, 7); // "hello 🌍"
/// assert!(truncated.chars().count() <= 7);
/// ```
#[inline]
pub fn truncate_str(s: &str, max_chars: usize) -> &str {
    if s.chars().count() <= max_chars {
        s
    } else {
        // Find the byte index that corresponds to max_chars characters
        let byte_idx = s.char_indices()
            .nth(max_chars)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len());
        &s[..byte_idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_ascii() {
        assert_eq!(truncate_str("hello world", 5), "hello");
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("", 5), "");
    }

    #[test]
    fn test_truncate_unicode() {
        // Japanese characters (3 bytes each)
        let japanese = "こんにちは";
        assert_eq!(truncate_str(japanese, 3), "こんに");

        // Emojis (4 bytes each)
        let emoji = "hello 🌍 world";
        let truncated = truncate_str(emoji, 7);
        assert_eq!(truncated.chars().count(), 7);

        // Mixed content
        let mixed = "café ☕";
        assert_eq!(truncate_str(mixed, 5), "café ");
    }

    #[test]
    fn test_truncate_exact_boundary() {
        assert_eq!(truncate_str("hello", 5), "hello");
        assert_eq!(truncate_str("hello", 0), "");
    }
}
