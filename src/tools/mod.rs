use anyhow::Result;

pub mod finalize;
pub mod first_answer;
pub mod peer_review;
pub mod save_review;
pub mod save_summary;
pub mod summarize;

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
}
