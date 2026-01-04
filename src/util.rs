/// Truncate a string to at most `max` characters, adding "..." if truncated.
pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }

    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    if max <= 3 {
        return ".".repeat(max);
    }

    let mut out: String = s.chars().take(max - 3).collect();
    out.push_str("...");
    out
}
