use crate::config;

/// Returns `true` if `name` satisfies identifier rules:
/// starts with an ASCII letter or `_`, followed by letters, digits, or `_`,
/// and is at most [`config::MAX_IDENTIFIER_LEN`] characters long.
pub fn is_valid_identifier(name: &str) -> bool {
    if name.is_empty() || name.len() > config::MAX_IDENTIFIER_LEN {
        return false;
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => false,
    }
}
