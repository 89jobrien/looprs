/// Walk up from `cwd` looking for `.env.nu`; source it via `nu` and inject
/// any vars it sets that aren't already in the process environment.
/// Silently no-ops if `nu` is absent or no `.env.nu` is found.
pub(crate) fn load_nu_env() {
    let mut dir = std::env::current_dir().unwrap_or_default();
    loop {
        let candidate = dir.join(".env.nu");
        if candidate.is_file() {
            apply_nu_env(&candidate);
            return;
        }
        if !dir.pop() {
            return;
        }
    }
}

fn apply_nu_env(path: &std::path::Path) {
    // Emit string-typed env vars as KEY=VALUE lines — avoids JSON control-char issues.
    let script = format!(
        "source '{}'; $env | items {{|k,v| if ($v | describe) == 'string' {{ $\"($k)=($v)\" }} }} | compact | str join (char newline)",
        path.display()
    );
    let Ok(output) = std::process::Command::new("nu")
        .args(["--no-config-file", "-c", &script])
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let Ok(text) = std::str::from_utf8(&output.stdout) else {
        return;
    };
    for line in text.lines() {
        if let Some((key, val)) = line.split_once('=')
            && std::env::var(key).is_err()
        {
            // SAFETY: single-threaded at this point in startup; no other
            // threads are reading the environment yet.
            unsafe { std::env::set_var(key, val) };
        }
    }
}
