//! Agent On Rails desktop setup wizard (AOR-008).
//! Thin onboarding only — day-to-day work stays in the `aor` CLI/TUI.

use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

const CLI_GIT: &str = "git+https://github.com/agent-on-rails/agent-on-rails-cli.git";
const DOCS_URL: &str = "https://agent-on-rails.suherman.net/docs/";
const WALKTHROUGH_URL: &str =
    "https://github.com/agent-on-rails/agent-on-rails-cli/blob/main/docs/walkthrough.md";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prerequisites {
    pub platform: String,
    pub python_path: Option<String>,
    pub python_version: Option<String>,
    pub python_ok: bool,
    pub pipx_available: bool,
    pub aor_path: Option<String>,
    pub aor_version: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub hint: Option<String>,
}

fn which(cmd: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        Command::new("where")
            .arg(cmd)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .map(|s| PathBuf::from(s.trim()))
            })
    }
    #[cfg(not(windows))]
    {
        Command::new("which")
            .arg(cmd)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if p.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(p))
                }
            })
    }
}

fn run_capture(program: &str, args: &[&str]) -> CommandResult {
    match Command::new(program).args(args).output() {
        Ok(out) => CommandResult {
            ok: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            hint: None,
        },
        Err(e) => CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: e.to_string(),
            hint: Some(format!("Failed to start `{program}`.")),
        },
    }
}

fn python_candidates() -> Vec<&'static str> {
    #[cfg(windows)]
    {
        vec!["python3", "python", "py"]
    }
    #[cfg(not(windows))]
    {
        vec!["python3", "python"]
    }
}

fn parse_python_version(text: &str) -> Option<(u32, u32)> {
    // "Python 3.14.6" or "Python 3.11.0"
    let rest = text.trim().strip_prefix("Python ")?;
    let mut parts = rest.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    Some((major, minor))
}

fn find_python() -> (Option<String>, Option<String>, bool) {
    for cand in python_candidates() {
        let Some(path) = which(cand) else { continue };
        let ver = if cand == "py" {
            run_capture("py", &["-3", "--version"])
        } else {
            run_capture(path.to_str().unwrap_or(cand), &["--version"])
        };
        let version_text = if !ver.stdout.is_empty() {
            ver.stdout
        } else {
            ver.stderr
        };
        if let Some((maj, min)) = parse_python_version(&version_text) {
            let ok = maj > 3 || (maj == 3 && min >= 11);
            return (
                Some(path.display().to_string()),
                Some(version_text.trim().to_string()),
                ok,
            );
        }
    }
    (None, None, false)
}

#[tauri::command]
fn check_prerequisites() -> Prerequisites {
    let mut notes = Vec::new();
    let (python_path, python_version, python_ok) = find_python();
    if !python_ok {
        notes.push(
            "Python 3.11+ is required. Install from https://www.python.org/downloads/ then re-check."
                .into(),
        );
    }

    let pipx_available = which("pipx").is_some()
        || python_path.as_ref().is_some_and(|p| {
            run_capture(p, &["-m", "pipx", "--version"]).ok
        });

    if python_ok && !pipx_available {
        notes.push(
            "pipx not found — install will fall back to `python -m pip install --user`.".into(),
        );
    }

    let aor_path = which("aor").map(|p| p.display().to_string());
    let aor_version = aor_path.as_ref().and_then(|_| {
        let r = run_capture("aor", &["--version"]);
        if r.ok && !r.stdout.is_empty() {
            Some(r.stdout)
        } else if !r.stderr.is_empty() {
            Some(r.stderr)
        } else {
            None
        }
    });

    Prerequisites {
        platform: std::env::consts::OS.to_string(),
        python_path,
        python_version,
        python_ok,
        pipx_available,
        aor_path,
        aor_version,
        notes,
    }
}

#[tauri::command]
fn install_cli() -> CommandResult {
    let (python_path, _, python_ok) = find_python();
    if !python_ok {
        return CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: "Python 3.11+ not found.".into(),
            hint: Some("Install Python 3.11+, then try again.".into()),
        };
    }
    let python = python_path.unwrap_or_else(|| "python3".into());

    let result = if which("pipx").is_some() {
        run_capture("pipx", &["install", "--force", CLI_GIT])
    } else if run_capture(&python, &["-m", "pipx", "--version"]).ok {
        run_capture(&python, &["-m", "pipx", "install", "--force", CLI_GIT])
    } else {
        let mut r = run_capture(
            &python,
            &["-m", "pip", "install", "--user", "--upgrade", CLI_GIT],
        );
        if r.ok {
            r.hint = Some(
                "Installed with pip --user. If `aor` is missing from PATH, open a new terminal or add your user Scripts directory."
                    .into(),
            );
        }
        r
    };

    if result.ok {
        let ver = run_capture("aor", &["--version"]);
        let mut combined = result;
        if ver.ok {
            combined.stdout = format!(
                "{}\n{}",
                combined.stdout,
                if ver.stdout.is_empty() {
                    ver.stderr
                } else {
                    ver.stdout
                }
            )
            .trim()
            .to_string();
        } else if combined.hint.is_none() {
            combined.hint = Some(
                "Package installed, but `aor` is not on PATH yet. Open a new terminal and retry, or run: python3 -m aor_cli"
                    .into(),
            );
        }
        return combined;
    }
    result
}

#[tauri::command]
fn init_project(path: String, name: String, force: bool) -> CommandResult {
    let path = path.trim().to_string();
    let name = name.trim().to_string();
    if path.is_empty() {
        return CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: "Choose a project folder.".into(),
            hint: None,
        };
    }
    if name.is_empty() {
        return CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: "Enter a product name.".into(),
            hint: None,
        };
    }

    let mut args = vec!["init".to_string(), path.clone(), "--name".to_string(), name];
    if force {
        args.push("--force".to_string());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

    if which("aor").is_some() {
        let mut r = run_capture("aor", &arg_refs);
        if r.ok {
            r.hint = Some(format!(
                "Next: open a terminal in `{path}`, run `aor`, then follow the walkthrough."
            ));
        }
        return r;
    }

    // Fallback: python -m aor_cli
    let (python_path, _, python_ok) = find_python();
    if !python_ok {
        return CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: "`aor` not found and Python 3.11+ is missing.".into(),
            hint: Some("Install the CLI on the previous step first.".into()),
        };
    }
    let python = python_path.unwrap_or_else(|| "python3".into());
    let mut module_args = vec!["-m", "aor_cli"];
    module_args.extend(arg_refs);
    let mut r = run_capture(&python, &module_args);
    if r.ok {
        r.hint = Some(format!(
            "Next: open a terminal in `{path}`, run `aor` (or `python3 -m aor_cli`), then follow the walkthrough."
        ));
    } else if r.hint.is_none() {
        r.hint = Some("Install or upgrade the CLI, then retry.".into());
    }
    r
}

#[tauri::command]
fn links() -> serde_json::Value {
    serde_json::json!({
        "docs": DOCS_URL,
        "walkthrough": WALKTHROUGH_URL,
        "cliRepo": "https://github.com/agent-on-rails/agent-on-rails-cli",
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            check_prerequisites,
            install_cli,
            init_project,
            links
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_python_versions() {
        assert_eq!(parse_python_version("Python 3.11.0"), Some((3, 11)));
        assert_eq!(parse_python_version("Python 3.14.6"), Some((3, 14)));
        assert_eq!(parse_python_version("Python 3.10.12"), Some((3, 10)));
        assert_eq!(parse_python_version("nope"), None);
    }

    #[test]
    fn python_311_is_ok() {
        let (_, _, ok_new) = {
            // unit-level: version gate
            let ok = matches!(parse_python_version("Python 3.11.0"), Some((3, m)) if m >= 11)
                || matches!(parse_python_version("Python 3.11.0"), Some((m, _)) if m > 3);
            ((), (), ok)
        };
        assert!(ok_new);
        let old = parse_python_version("Python 3.10.0").unwrap();
        assert!(!(old.0 > 3 || (old.0 == 3 && old.1 >= 11)));
    }
}
