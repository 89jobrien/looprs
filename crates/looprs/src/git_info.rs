use std::process::Command;

#[derive(Debug, Default, Clone)]
pub struct GitInfo {
    pub branch: Option<String>,
    pub ahead: u32,
    pub modified: u32,
    pub untracked: u32,
}

pub fn collect() -> GitInfo {
    let branch = branch_name();
    let ahead = commits_ahead();
    let (modified, untracked) = changed_files();
    GitInfo {
        branch,
        ahead,
        modified,
        untracked,
    }
}

fn branch_name() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() || s == "HEAD" {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}

fn commits_ahead() -> u32 {
    let out = Command::new("git")
        .args(["rev-list", "--count", "@{u}..HEAD"])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .trim()
            .parse()
            .unwrap_or(0),
        _ => 0,
    }
}

fn changed_files() -> (u32, u32) {
    let out = Command::new("git").args(["status", "--porcelain"]).output();
    match out {
        Ok(o) if o.status.success() => {
            let mut modified = 0u32;
            let mut untracked = 0u32;
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                if line.starts_with("??") {
                    untracked += 1;
                } else if !line.is_empty() {
                    modified += 1;
                }
            }
            (modified, untracked)
        }
        _ => (0, 0),
    }
}
