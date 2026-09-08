use anyhow::Result;

pub mod finalize;
pub mod first_answer;
pub mod peer_review;
pub mod save_review;
pub mod save_summary;
pub mod summarize;

/// Windows reserves these device names in any directory; `fs::write` onto
/// them fails with a confusing OS error, so reject the stem up front.
const WINDOWS_RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Validate a conversation `title` before it is joined onto `$HOME/.council`.
///
/// Titles flow straight into `PathBuf::join`, which replaces the whole base
/// for absolute paths and honors `..`, so an unchecked title allows writes
/// anywhere on the filesystem. Only a single plain directory name is allowed.
pub fn sanitize_title(raw: &str) -> Result<&str> {
    let title = raw.trim();
    if title.is_empty() {
        anyhow::bail!("title must not be empty");
    }
    if title.starts_with('.')
        || title.contains("..")
        || title.contains('\\')
        || title.chars().any(std::path::is_separator)
    {
        anyhow::bail!(
            "title must be a plain directory name without path separators or dot segments: {title:?}"
        );
    }
    // Reserved device names match up to the first dot ("con.txt" too) and are
    // case-insensitive on Windows.
    let stem = title
        .split('.')
        .next()
        .unwrap_or(title)
        .to_ascii_lowercase();
    if WINDOWS_RESERVED.contains(&stem.as_str()) {
        anyhow::bail!("title uses a reserved Windows device name: {title:?}");
    }
    Ok(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_title_accepts_plain_names() {
        assert_eq!(sanitize_title("my-topic").unwrap(), "my-topic");
        assert_eq!(sanitize_title("  council_1  ").unwrap(), "council_1");
        assert_eq!(sanitize_title("회의-노트").unwrap(), "회의-노트");
    }

    #[test]
    fn sanitize_title_rejects_traversal() {
        assert!(sanitize_title("../etc").is_err());
        assert!(sanitize_title("a/b").is_err());
        assert!(sanitize_title("a\\b").is_err());
        assert!(sanitize_title("/tmp/evil").is_err());
        assert!(sanitize_title(".").is_err());
        assert!(sanitize_title("..").is_err());
        assert!(sanitize_title("").is_err());
    }

    #[test]
    fn sanitize_title_rejects_windows_device_names() {
        assert!(sanitize_title("con").is_err());
        assert!(sanitize_title("NUL").is_err());
        assert!(sanitize_title("aux.txt").is_err());
        assert!(sanitize_title("Com1").is_err());
        // Not reserved: device-like stem beyond the reserved set.
        assert!(sanitize_title("console").is_ok());
        assert!(sanitize_title("null-byte-research").is_ok());
    }
}
