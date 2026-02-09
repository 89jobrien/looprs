use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Resolve file references in text (e.g., @file.rs -> file contents)
pub fn resolve_file_references(text: &str, base_dir: &Path) -> Result<String> {
    let mut result = String::new();
    let mut last_end = 0;

    // Find all @filename references
    for (start, reference) in find_file_references(text) {
        // Add text before this reference
        result.push_str(&text[last_end..start]);

        // Resolve the reference
        match resolve_reference(&reference, base_dir) {
            Ok(content) => {
                result.push_str(&format!("\n```\n// File: {reference}\n{content}\n```\n"));
            }
            Err(e) => {
                // If file not found, keep the original reference and add error note
                result.push_str(&format!("@{reference}"));
                crate::ui::warn(format!("Warning: Could not resolve @{reference}: {e}"));
            }
        }

        last_end = start + reference.len() + 1; // +1 for the @ symbol
    }

    // Add remaining text
    result.push_str(&text[last_end..]);

    Ok(result)
}

/// Find all file references in text (returns position and filename)
fn find_file_references(text: &str) -> Vec<(usize, String)> {
    let mut references = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '@' {
            // Check if this is at word boundary (start of line or after space/punctuation)
            let at_boundary = i == 0
                || chars[i - 1].is_whitespace()
                || chars[i - 1] == '('
                || chars[i - 1] == '['
                || chars[i - 1] == '{'
                || chars[i - 1] == ',';

            if at_boundary {
                // Extract filename (alphanumeric, dash, underscore, dot, slash)
                let start = i;
                i += 1;
                let mut filename = String::new();

                while i < chars.len() {
                    let c = chars[i];
                    if c.is_alphanumeric()
                        || c == '-'
                        || c == '_'
                        || c == '.'
                        || c == '/'
                        || c == '\\'
                    {
                        filename.push(c);
                        i += 1;
                    } else {
                        break;
                    }
                }

                if !filename.is_empty() {
                    references.push((start, filename));
                    continue;
                }
            }
        }
        i += 1;
    }

    references
}

/// Resolve a single file reference
fn resolve_reference(filename: &str, base_dir: &Path) -> Result<String> {
    let path = base_dir.join(filename);

    // Security: prevent path traversal attacks
    let canonical = path
        .canonicalize()
        .with_context(|| format!("File not found: {filename}"))?;

    let base_canonical = base_dir
        .canonicalize()
        .unwrap_or_else(|_| base_dir.to_path_buf());

    if !canonical.starts_with(&base_canonical) {
        anyhow::bail!("Access denied: {filename} is outside working directory");
    }

    // Read file contents
    let content = fs::read_to_string(&canonical)
        .with_context(|| format!("Failed to read file: {filename}"))?;

    Ok(content)
}

/// Check if text contains any file references
pub fn has_file_references(text: &str) -> bool {
    !find_file_references(text).is_empty()
}

/// List all file references in text
pub fn list_file_references(text: &str) -> Vec<String> {
    find_file_references(text)
        .into_iter()
        .map(|(_, filename)| filename)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, filename: &str, content: &str) {
        let path = dir.join(filename);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = fs::File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
    }

    #[test]
    fn test_find_file_references() {
        let refs = find_file_references("Check @file.rs and @another.txt");
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].1, "file.rs");
        assert_eq!(refs[1].1, "another.txt");
    }

    #[test]
    fn test_no_file_references() {
        let refs = find_file_references("No references here");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_email_not_a_reference() {
        // email@domain.com should not be treated as a file reference
        // The @ in an email has a letter before it, not whitespace/punctuation
        let refs = find_file_references("Contact me at user@domain.com for help");
        // Should not find any references (@ is preceded by a letter)
        assert_eq!(refs.len(), 0);
    }

    #[test]
    fn test_reference_with_path() {
        let refs = find_file_references("Check @src/main.rs");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].1, "src/main.rs");
    }

    #[test]
    fn test_resolve_file_references() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "test.txt", "Hello, world!");

        let input = "Check @test.txt please";
        let result = resolve_file_references(input, temp_dir.path()).unwrap();

        assert!(result.contains("Hello, world!"));
        assert!(result.contains("// File: test.txt"));
    }

    #[test]
    fn test_resolve_multiple_references() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "file1.txt", "Content 1");
        create_test_file(temp_dir.path(), "file2.txt", "Content 2");

        let input = "Compare @file1.txt and @file2.txt";
        let result = resolve_file_references(input, temp_dir.path()).unwrap();

        assert!(result.contains("Content 1"));
        assert!(result.contains("Content 2"));
    }

    #[test]
    fn test_resolve_with_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "src/lib.rs", "pub fn main() {}");

        let input = "Review @src/lib.rs";
        let result = resolve_file_references(input, temp_dir.path()).unwrap();

        assert!(result.contains("pub fn main()"));
    }

    #[test]
    fn test_missing_file_keeps_reference() {
        let temp_dir = TempDir::new().unwrap();

        let input = "Check @nonexistent.txt";
        let result = resolve_file_references(input, temp_dir.path()).unwrap();

        // Should keep the original reference when file not found
        assert!(result.contains("@nonexistent.txt"));
    }

    #[test]
    fn test_has_file_references() {
        assert!(has_file_references("Check @file.rs"));
        assert!(!has_file_references("No references here"));
    }

    #[test]
    fn test_list_file_references() {
        let refs = list_file_references("See @a.txt and @b.rs and @c.md");
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0], "a.txt");
        assert_eq!(refs[1], "b.rs");
        assert_eq!(refs[2], "c.md");
    }

    #[test]
    fn test_path_traversal_blocked() {
        let temp_dir = TempDir::new().unwrap();

        // Try to escape the working directory
        let result = resolve_reference("../../../etc/passwd", temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_reference_at_start_of_line() {
        let refs = find_file_references("@file.rs\nAnother line");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].1, "file.rs");
    }

    #[test]
    fn test_reference_after_punctuation() {
        let refs = find_file_references("Files: (@a.txt, @b.rs)");
        assert_eq!(refs.len(), 2);
    }
}
