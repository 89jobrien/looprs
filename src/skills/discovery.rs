use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillFrontmatterSummary {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSkill {
    pub path: PathBuf,
    pub skill_file: PathBuf,
    pub name: String,
    pub description: String,
    pub source_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSkillPath {
    pub skill_file: PathBuf,
    pub source_type: String,
    pub skill_path: String,
}

pub fn extract_frontmatter(file_path: &Path) -> SkillFrontmatterSummary {
    let Ok(content) = fs::read_to_string(file_path) else {
        return SkillFrontmatterSummary::default();
    };
    extract_frontmatter_from_content(&content)
}

pub fn extract_frontmatter_from_content(content: &str) -> SkillFrontmatterSummary {
    let mut in_frontmatter = false;
    let mut name = String::new();
    let mut description = String::new();
    let key_re = Regex::new(r"^(\w+):\s*(.*)$").expect("valid frontmatter regex");

    for line in content.lines() {
        if line.trim() == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }

        if !in_frontmatter {
            continue;
        }

        if let Some(captures) = key_re.captures(line) {
            let key = captures.get(1).map(|m| m.as_str()).unwrap_or_default();
            let value = captures
                .get(2)
                .map(|m| m.as_str())
                .unwrap_or_default()
                .trim();
            match key {
                "name" => name = value.to_string(),
                "description" => description = value.to_string(),
                _ => {}
            }
        }
    }

    SkillFrontmatterSummary { name, description }
}

pub fn find_skills_in_dir(dir: &Path, source_type: &str, max_depth: usize) -> Vec<DiscoveredSkill> {
    let mut skills = Vec::new();
    if !dir.exists() {
        return skills;
    }
    recurse_skills(dir, source_type, 0, max_depth, &mut skills);
    skills
}

pub fn resolve_skill_path(
    skill_name: &str,
    superpowers_dir: Option<&Path>,
    personal_dir: Option<&Path>,
) -> Option<ResolvedSkillPath> {
    let force_superpowers = skill_name.starts_with("superpowers:");
    let actual_skill_name = if force_superpowers {
        skill_name.trim_start_matches("superpowers:")
    } else {
        skill_name
    };

    if !force_superpowers
        && let Some(personal) = personal_dir
    {
        let skill_path = personal.join(actual_skill_name);
        let skill_file = skill_path.join("SKILL.md");
        if skill_file.exists() {
            return Some(ResolvedSkillPath {
                skill_file,
                source_type: "personal".to_string(),
                skill_path: actual_skill_name.to_string(),
            });
        }
    }

    if let Some(superpowers) = superpowers_dir {
        let skill_path = superpowers.join(actual_skill_name);
        let skill_file = skill_path.join("SKILL.md");
        if skill_file.exists() {
            return Some(ResolvedSkillPath {
                skill_file,
                source_type: "superpowers".to_string(),
                skill_path: actual_skill_name.to_string(),
            });
        }
    }

    None
}

pub fn check_for_updates(repo_dir: &Path) -> bool {
    let mut child = match Command::new("sh")
        .arg("-c")
        .arg("git fetch origin && git status --porcelain=v1 --branch")
        .current_dir(repo_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };

    let start = Instant::now();
    let timeout = Duration::from_secs(3);

    loop {
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }

        match child.try_wait() {
            Ok(Some(_status)) => match child.wait_with_output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return stdout
                        .lines()
                        .any(|line| line.starts_with("## ") && line.contains("[behind "));
                }
                Err(_) => return false,
            },
            Ok(None) => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return false,
        }
    }
}

pub fn strip_frontmatter(content: &str) -> String {
    let mut in_frontmatter = false;
    let mut frontmatter_ended = false;
    let mut content_lines = Vec::new();

    for line in content.lines() {
        if line.trim() == "---" {
            if in_frontmatter {
                frontmatter_ended = true;
                continue;
            }
            in_frontmatter = true;
            continue;
        }

        if frontmatter_ended || !in_frontmatter {
            content_lines.push(line);
        }
    }

    content_lines.join("\n").trim().to_string()
}

fn recurse_skills(
    current_dir: &Path,
    source_type: &str,
    depth: usize,
    max_depth: usize,
    skills: &mut Vec<DiscoveredSkill>,
) {
    if depth > max_depth {
        return;
    }

    let Ok(entries) = fs::read_dir(current_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let full_path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };

        if !ft.is_dir() {
            continue;
        }

        let skill_file = full_path.join("SKILL.md");
        if skill_file.exists() {
            let frontmatter = extract_frontmatter(&skill_file);
            let fallback_name = full_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();

            skills.push(DiscoveredSkill {
                path: full_path.clone(),
                skill_file: skill_file.clone(),
                name: if frontmatter.name.is_empty() {
                    fallback_name
                } else {
                    frontmatter.name
                },
                description: frontmatter.description,
                source_type: source_type.to_string(),
            });
        }

        recurse_skills(&full_path, source_type, depth + 1, max_depth, skills);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extracts_frontmatter_name_and_description() {
        let content = r#"---
name: rust-testing
description: Use when writing tests
triggers:
  - cargo test
---

# Content
"#;

        let fm = extract_frontmatter_from_content(content);
        assert_eq!(fm.name, "rust-testing");
        assert_eq!(fm.description, "Use when writing tests");
    }

    #[test]
    fn strips_frontmatter_from_content() {
        let content = r#"---
name: sample
description: desc
---

# Body
text
"#;

        let stripped = strip_frontmatter(content);
        assert_eq!(stripped, "# Body\ntext");
    }

    #[test]
    fn find_skills_matches_js_max_depth_behavior() {
        let temp = TempDir::new().expect("tempdir");
        let level1 = temp.path().join("a");
        let level2 = level1.join("b");
        let level3 = level2.join("c");
        let level4 = level3.join("d");

        fs::create_dir_all(&level4).expect("create dirs");

        fs::write(
            level1.join("SKILL.md"),
            "---\nname: one\ndescription: d\n---\ncontent",
        )
        .expect("write skill 1");
        fs::write(
            level4.join("SKILL.md"),
            "---\nname: four\ndescription: d\n---\ncontent",
        )
        .expect("write skill 4");

        let found = find_skills_in_dir(temp.path(), "internal", 3);
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|s| s.name == "one"));
        assert!(found.iter().any(|s| s.name == "four"));
        assert!(found.iter().all(|s| s.source_type == "internal"));
    }

    #[test]
    fn resolve_skill_path_prefers_personal_unless_forced() {
        let personal = TempDir::new().expect("tempdir personal");
        let superpowers = TempDir::new().expect("tempdir superpowers");

        let skill_name = "brainstorm";
        let personal_skill_dir = personal.path().join(skill_name);
        let super_skill_dir = superpowers.path().join(skill_name);

        fs::create_dir_all(&personal_skill_dir).expect("create personal dir");
        fs::create_dir_all(&super_skill_dir).expect("create super dir");

        fs::write(personal_skill_dir.join("SKILL.md"), "---\nname: p\n---\n")
            .expect("write personal skill");
        fs::write(super_skill_dir.join("SKILL.md"), "---\nname: s\n---\n")
            .expect("write super skill");

        let resolved =
            resolve_skill_path(skill_name, Some(superpowers.path()), Some(personal.path()))
                .expect("resolved personal");
        assert_eq!(resolved.source_type, "personal");

        let forced = resolve_skill_path(
            "superpowers:brainstorm",
            Some(superpowers.path()),
            Some(personal.path()),
        )
        .expect("resolved superpowers");
        assert_eq!(forced.source_type, "superpowers");
    }
}
