//! Agent On Rails desktop setup wizard (AOR-008).
//! Thin onboarding only — day-to-day work stays in the `aor` CLI/TUI.

use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

/// HTTP sdist — avoids `git clone` / `git-remote-https` (broken Git at /usr/local/bin).
const CLI_ARCHIVE: &str =
    "https://github.com/agent-on-rails/agent-on-rails-cli/archive/refs/heads/main.tar.gz";
const CLI_GIT: &str = "git+https://github.com/agent-on-rails/agent-on-rails-cli.git";
const CLI_VERSION_URL: &str =
    "https://raw.githubusercontent.com/agent-on-rails/agent-on-rails-cli/main/src/aor_cli/__init__.py";
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
    pub aor_latest_version: Option<String>,
    pub aor_update_available: bool,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatherExtractResult {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub hint: Option<String>,
    pub outline: Option<serde_json::Value>,
    pub outline_path: Option<String>,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn extra_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = home_dir() {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join("bin"));
        #[cfg(windows)]
        {
            dirs.push(home.join(r"AppData\Roaming\Python\Python314\Scripts"));
            dirs.push(home.join(r"AppData\Roaming\Python\Python313\Scripts"));
            dirs.push(home.join(r"AppData\Roaming\Python\Python312\Scripts"));
        }
        #[cfg(not(windows))]
        {
            dirs.push(home.join("Library/Python/3.14/bin"));
            dirs.push(home.join("Library/Python/3.13/bin"));
            dirs.push(home.join("Library/Python/3.12/bin"));
        }
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/opt/homebrew/sbin"));
    dirs.push(PathBuf::from("/usr/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

fn path_separator() -> &'static str {
    #[cfg(windows)]
    {
        ";"
    }
    #[cfg(not(windows))]
    {
        ":"
    }
}

fn augmented_path() -> String {
    let sep = path_separator();
    let mut parts: Vec<String> = extra_bin_dirs()
        .into_iter()
        .map(|p| p.display().to_string())
        .collect();
    if let Ok(existing) = std::env::var("PATH") {
        parts.push(existing);
    }
    parts.join(sep)
}

fn working_git_exec_path() -> Option<String> {
    let candidates = ["/opt/homebrew/bin/git", "/usr/bin/git", "git"];
    for git in candidates {
        if git.starts_with('/') && !Path::new(git).exists() {
            continue;
        }
        let out = match Command::new(git).arg("--exec-path").output() {
            Ok(o) => o,
            Err(_) => continue,
        };
        if !out.status.success() {
            continue;
        }
        let exec = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if exec.is_empty() {
            continue;
        }
        let helper = Path::new(&exec).join("git-remote-https");
        let helper_exe = Path::new(&exec).join("git-remote-https.exe");
        if helper.exists() || helper_exe.exists() {
            return Some(exec);
        }
    }
    None
}

fn apply_env(cmd: &mut Command) {
    cmd.env("PATH", augmented_path());
    cmd.env("PYTHONUNBUFFERED", "1");
    cmd.env("PIP_DISABLE_PIP_VERSION_CHECK", "1");
    cmd.env("PIP_PROGRESS_BAR", "on");
    if let Some(exec) = working_git_exec_path() {
        cmd.env("GIT_EXEC_PATH", exec);
    }
}

fn which(cmd: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    let (finder, flag) = ("where", cmd);
    #[cfg(not(windows))]
    let finder = "which";

    let mut c = Command::new(finder);
    apply_env(&mut c);
    #[cfg(windows)]
    c.arg(flag);
    #[cfg(not(windows))]
    c.arg(cmd);

    c.output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
        })
}

fn run_capture(program: &str, args: &[&str]) -> CommandResult {
    let mut cmd = Command::new(program);
    apply_env(&mut cmd);
    match cmd.args(args).output() {
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

fn emit_install(app: &AppHandle, stage: &str, percent: u8, line: &str, done: bool) {
    let _ = app.emit(
        "install-progress",
        serde_json::json!({
            "stage": stage,
            "percent": percent.min(100),
            "line": line,
            "done": done,
        }),
    );
}

fn percent_for_install_line(line: &str, current: u8) -> u8 {
    let l = line.to_lowercase();
    let mapped = if l.contains("downloading") || l.contains("% total") || l.contains("  % ") {
        parse_percent(line).unwrap_or(40)
    } else if l.contains("determining package") {
        42
    } else if l.contains("creating virtual") {
        55
    } else if l.contains("installing") || l.contains("resolving") {
        72
    } else if l.contains("installed package") || l.contains("done.") {
        90
    } else {
        current.saturating_add(1).min(88)
    };
    mapped.max(current).min(95)
}

fn parse_percent(line: &str) -> Option<u8> {
    let trimmed = line.trim();
    let idx = trimmed.rfind('%')?;
    let prefix = trimmed[..idx].trim();
    let num = prefix.split_whitespace().last()?;
    let value: f32 = num.parse().ok()?;
    Some(value.clamp(0.0, 100.0) as u8)
}

fn pump_install_stream<R: std::io::Read + Send + 'static>(
    stream: R,
    app: AppHandle,
    stage: String,
    log: Arc<Mutex<String>>,
    pct: Arc<Mutex<u8>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let line = line.trim_end().to_string();
            if line.is_empty() {
                continue;
            }
            let next = {
                let mut p = pct.lock().unwrap();
                *p = percent_for_install_line(&line, *p);
                *p
            };
            if let Ok(mut g) = log.lock() {
                g.push_str(&line);
                g.push('\n');
            }
            emit_install(&app, &stage, next, &line, false);
        }
    })
}

fn stream_command(app: &AppHandle, mut cmd: Command, stage: &str, start_pct: u8) -> CommandResult {
    apply_env(&mut cmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CommandResult {
                ok: false,
                stdout: String::new(),
                stderr: e.to_string(),
                hint: Some(format!("Failed to start {stage}.")),
            };
        }
    };

    let log = Arc::new(Mutex::new(String::new()));
    let pct = Arc::new(Mutex::new(start_pct));
    let mut pumps = Vec::new();
    if let Some(out) = child.stdout.take() {
        pumps.push(pump_install_stream(
            out,
            app.clone(),
            stage.to_string(),
            Arc::clone(&log),
            Arc::clone(&pct),
        ));
    }
    if let Some(err) = child.stderr.take() {
        pumps.push(pump_install_stream(
            err,
            app.clone(),
            stage.to_string(),
            Arc::clone(&log),
            Arc::clone(&pct),
        ));
    }

    for t in pumps {
        let _ = t.join();
    }
    let status = child.wait();
    let text = log.lock().ok().map(|g| g.trim().to_string()).unwrap_or_default();
    match status {
        Ok(st) => CommandResult {
            ok: st.success(),
            stdout: text,
            stderr: String::new(),
            hint: None,
        },
        Err(e) => CommandResult {
            ok: false,
            stdout: text,
            stderr: e.to_string(),
            hint: Some(format!("{stage} failed.")),
        },
    }
}

fn download_cli_archive(app: &AppHandle, dest: &Path) -> CommandResult {
    emit_install(
        app,
        "Downloading CLI",
        8,
        &format!("GET {CLI_ARCHIVE}"),
        false,
    );
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let dest_s = dest.display().to_string();
    if which("curl").is_some() {
        let mut cmd = Command::new("curl");
        cmd.args([
            "-L",
            "--fail",
            "--retry",
            "3",
            "--no-buffer",
            "--progress-bar",
            "-A",
            "agent-on-rails-desktop",
            "-o",
            &dest_s,
            CLI_ARCHIVE,
        ]);
        let r = stream_command(app, cmd, "Downloading CLI", 10);
        if r.ok && dest.is_file() {
            return r;
        }
        emit_install(app, "Downloading CLI", 15, "curl failed; trying Python…", false);
    }

    let (python, _, python_ok) = find_python();
    if !python_ok {
        return CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: "Need curl or Python to download the CLI archive.".into(),
            hint: Some("Install Python 3.11+, then retry.".into()),
        };
    }
    let python = python.unwrap_or_else(|| "python3".into());
    let script = format!(
        "import urllib.request; urllib.request.urlretrieve({url:?}, {dest:?})",
        url = CLI_ARCHIVE,
        dest = dest_s,
    );
    let mut cmd = Command::new(python);
    cmd.args(["-u", "-c", &script]);
    stream_command(app, cmd, "Downloading CLI", 20)
}

fn run_aor(args: &[String]) -> CommandResult {
    run_aor_with_stdin(args, None)
}

fn sibling_cli_root() -> Option<PathBuf> {
    let from_env = std::env::var("AOR_CLI_SRC").ok().map(PathBuf::from);
    let from_cargo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../agent-on-rails-cli");
    for cand in [from_env, Some(from_cargo)].into_iter().flatten() {
        if cand.join("src/aor_cli/__main__.py").is_file() {
            return cand.canonicalize().ok().or(Some(cand));
        }
    }
    None
}

fn sibling_venv_aor(cli_root: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    let cand = cli_root.join(r".venv\Scripts\aor.exe");
    #[cfg(not(windows))]
    let cand = cli_root.join(".venv/bin/aor");
    cand.is_file().then_some(cand)
}

fn sibling_venv_python(cli_root: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    let cand = cli_root.join(r".venv\Scripts\python.exe");
    #[cfg(not(windows))]
    let cand = cli_root.join(".venv/bin/python");
    cand.is_file().then_some(cand)
}

/// How to invoke the CLI. Prefer sibling checkout (dev) so gather uses local templates.
fn aor_launch() -> Result<(String, Vec<String>, Option<PathBuf>), CommandResult> {
    if let Some(cli_root) = sibling_cli_root() {
        let pythonpath = Some(cli_root.join("src"));
        if let Some(venv_aor) = sibling_venv_aor(&cli_root) {
            return Ok((venv_aor.display().to_string(), Vec::new(), pythonpath));
        }
        if let Some(venv_py) = sibling_venv_python(&cli_root) {
            return Ok((
                venv_py.display().to_string(),
                vec!["-m".into(), "aor_cli".into()],
                pythonpath,
            ));
        }
        let (python_path, _, python_ok) = find_python();
        if python_ok {
            return Ok((
                python_path.unwrap_or_else(|| "python3".into()),
                vec!["-m".into(), "aor_cli".into()],
                pythonpath,
            ));
        }
    }
    if let Some(aor) = which("aor") {
        return Ok((aor.display().to_string(), Vec::new(), None));
    }
    Err(CommandResult {
        ok: false,
        stdout: String::new(),
        stderr: "`aor` not found. Install the CLI first, or keep agent-on-rails-cli next to this app.".into(),
        hint: Some("Use the Install CLI step, then retry.".into()),
    })
}

fn run_aor_with_stdin(args: &[String], stdin: Option<&[u8]>) -> CommandResult {
    let (program, prefix, pythonpath) = match aor_launch() {
        Ok(launch) => launch,
        Err(err) => return err,
    };
    let mut argv = prefix;
    argv.extend(args.iter().cloned());

    let mut cmd = Command::new(&program);
    apply_env(&mut cmd);
    if let Some(pp) = pythonpath {
        cmd.env("PYTHONPATH", pp);
    }
    cmd.args(&argv);
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        return match cmd.spawn() {
            Ok(mut child) => {
                if let (Some(data), Some(mut pipe)) = (stdin, child.stdin.take()) {
                    let _ = pipe.write_all(data);
                }
                match child.wait_with_output() {
                    Ok(out) => command_result_from_output(&program, out),
                    Err(e) => CommandResult {
                        ok: false,
                        stdout: String::new(),
                        stderr: e.to_string(),
                        hint: Some("Failed to run `aor`.".into()),
                    },
                }
            }
            Err(e) => CommandResult {
                ok: false,
                stdout: String::new(),
                stderr: e.to_string(),
                hint: Some(format!("Failed to start `{program}`.")),
            },
        };
    }
    match cmd.output() {
        Ok(out) => command_result_from_output(&program, out),
        Err(e) => CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: e.to_string(),
            hint: Some(format!("Failed to start `{program}`.")),
        },
    }
}

fn command_result_from_output(_program: &str, out: std::process::Output) -> CommandResult {
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let hint = if !out.status.success()
        && (stderr.contains("No module named aor_cli") || stdout.contains("No module named aor_cli"))
    {
        Some("The selected Python does not have `aor`. Install the CLI, then retry.".into())
    } else {
        None
    };
    CommandResult {
        ok: out.status.success(),
        stdout,
        stderr,
        hint,
    }
}

fn gather_extract_args(root: &str, file: &str, stub: bool, outline_only: bool) -> Vec<String> {
    let mut args = vec![
        "gather".into(),
        "run".into(),
        "--root".into(),
        root.to_string(),
        "--file".into(),
        file.to_string(),
    ];
    if outline_only {
        args.push("--outline-only".into());
        args.push("--json".into());
    }
    if stub {
        args.push("--stub".into());
    }
    args
}

fn unknown_cli_option(r: &CommandResult) -> bool {
    let text = format!("{}\n{}", r.stdout, r.stderr).to_lowercase();
    text.contains("no such option") || text.contains("unrecognized arguments")
}

fn gather_apply_args(root: &str, force: bool) -> Vec<String> {
    let mut args = vec![
        "gather".into(),
        "apply".into(),
        "--root".into(),
        root.to_string(),
        "--yes".into(),
    ];
    if force {
        args.push("--force".into());
    }
    args
}

fn outline_json_path(root: &str) -> PathBuf {
    Path::new(root).join(".aor").join("gather").join("outline.json")
}

fn read_outline(root: &str) -> Option<serde_json::Value> {
    let text = fs::read_to_string(outline_json_path(root)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_outline(root: &str, value: &serde_json::Value) {
    let path = outline_json_path(root);
    if let Ok(text) = serde_json::to_string_pretty(value) {
        let _ = fs::write(path, format!("{text}\n"));
    }
}

const TITLE_TAIL_STOP: &[&str] = &[
    "a", "an", "the", "and", "or", "of", "for", "to", "with", "on", "in", "at", "by", "as",
];

fn strip_shall_prefix(shall: &str) -> &str {
    let t = shall.trim();
    for prefix in ["The system SHALL ", "The system shall ", "THE SYSTEM SHALL "] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return rest.trim();
        }
    }
    t
}

fn cap_title(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        None => "Requirement".into(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn trim_title_stopwords(title: &str) -> String {
    let mut words: Vec<&str> = title.split_whitespace().collect();
    while words.len() > 4 {
        let last = words
            .last()
            .unwrap_or(&"")
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_ascii_lowercase();
        if TITLE_TAIL_STOP.contains(&last.as_str()) {
            words.pop();
        } else {
            break;
        }
    }
    words.join(" ")
}

fn title_from_shall(shall: &str) -> String {
    let cleaned = strip_shall_prefix(shall).trim_end_matches(|c: char| ".,;:".contains(c));
    if cleaned.is_empty() {
        return "Requirement".into();
    }
    if cleaned.len() <= 100 {
        return cap_title(&trim_title_stopwords(cleaned));
    }
    let mut cut: String = cleaned.chars().take(100).collect();
    if let Some(i) = cut.rfind(' ') {
        cut.truncate(i);
    }
    cap_title(&trim_title_stopwords(&cut))
}

fn title_looks_truncated(title: &str) -> bool {
    let last = title
        .split_whitespace()
        .last()
        .unwrap_or("")
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_ascii_lowercase();
    TITLE_TAIL_STOP.contains(&last.as_str())
}

fn guess_product_name_from_text(text: &str) -> Option<String> {
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        if word.len() >= 5
            && word.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && (word.ends_with("Desk") || word.ends_with("Hub"))
        {
            return Some(word.to_string());
        }
    }
    None
}

fn polish_stub_outline(outline: &mut serde_json::Value, requirements: &str) {
    let weak_name = outline
        .get("product_name")
        .and_then(|v| v.as_str())
        .is_some_and(|n| {
            matches!(
                n.to_ascii_lowercase().as_str(),
                "called" | "named" | "product" | "build" | "building"
            )
        });
    if weak_name {
        if let Some(name) = guess_product_name_from_text(requirements).or_else(|| {
            outline
                .get("vision")
                .and_then(|v| v.as_str())
                .and_then(guess_product_name_from_text)
        }) {
            outline["product_name"] = serde_json::Value::String(name.clone());
            outline["tagline"] = serde_json::Value::String(format!(
                "{name} — drafted from natural-language requirements"
            ));
        }
    }
    let Some(reqs) = outline.get_mut("requirements").and_then(|r| r.as_array_mut()) else {
        return;
    };
    for req in reqs {
        let title = req
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let shall = req
            .get("shall")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        // Only rewrite truncated stub titles ("… called SurveyDesk: a"). Do not
        // replace short canonical titles such as "Product surfaces".
        if title_looks_truncated(&title) {
            let next = title_from_shall(&shall);
            if let Some(obj) = req.as_object_mut() {
                obj.insert("title".into(), serde_json::Value::String(next));
            }
        }
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

fn parse_semver(text: &str) -> Option<(u32, u32, u32)> {
    let mut buf = String::new();
    let mut parts: Vec<u32> = Vec::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            buf.push(ch);
        } else if ch == '.' {
            if buf.is_empty() {
                continue;
            }
            parts.push(buf.parse().ok()?);
            buf.clear();
            if parts.len() == 3 {
                break;
            }
        } else if !buf.is_empty() || !parts.is_empty() {
            break;
        }
    }
    if !buf.is_empty() && parts.len() < 3 {
        parts.push(buf.parse().ok()?);
    }
    match parts.as_slice() {
        [maj, min, pat] => Some((*maj, *min, *pat)),
        [maj, min] => Some((*maj, *min, 0)),
        [maj] => Some((*maj, 0, 0)),
        _ => None,
    }
}

fn parse_init_version(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("__version__") else {
            continue;
        };
        let rest = rest.trim().trim_start_matches('=').trim();
        let quoted = rest.trim_matches(['"', '\'']);
        if parse_semver(quoted).is_some() {
            return Some(quoted.to_string());
        }
    }
    None
}

fn version_newer(latest: &str, installed: &str) -> bool {
    match (parse_semver(latest), parse_semver(installed)) {
        (Some(l), Some(i)) => l > i,
        _ => false,
    }
}

fn fetch_latest_aor_version() -> Option<String> {
    if which("curl").is_none() {
        return None;
    }
    let mut cmd = Command::new("curl");
    apply_env(&mut cmd);
    cmd.args([
        "-fsSL",
        "--max-time",
        "5",
        "-A",
        "agent-on-rails-desktop",
        CLI_VERSION_URL,
    ]);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_init_version(&String::from_utf8_lossy(&out.stdout))
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
    let aor_latest_version = fetch_latest_aor_version();
    let aor_update_available = match (aor_path.as_ref(), aor_version.as_deref(), aor_latest_version.as_deref()) {
        (Some(_), Some(installed), Some(latest)) => version_newer(latest, installed),
        _ => false,
    };

    Prerequisites {
        platform: std::env::consts::OS.to_string(),
        python_path,
        python_version,
        python_ok,
        pipx_available,
        aor_path,
        aor_version,
        aor_latest_version,
        aor_update_available,
        notes,
    }
}

#[tauri::command]
async fn install_cli(app: AppHandle) -> CommandResult {
    tauri::async_runtime::spawn_blocking(move || install_cli_blocking(app))
        .await
        .unwrap_or_else(|e| CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: e.to_string(),
            hint: Some("Install task failed to run.".into()),
        })
}

fn install_cli_blocking(app: AppHandle) -> CommandResult {
    emit_install(&app, "Preparing", 3, "Checking Python 3.11+…", false);
    let (python_path, _, python_ok) = find_python();
    if !python_ok {
        emit_install(&app, "Failed", 0, "Python 3.11+ not found.", true);
        return CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: "Python 3.11+ not found.".into(),
            hint: Some("Install Python 3.11+, then try again.".into()),
        };
    }
    let python = python_path.unwrap_or_else(|| "python3".into());

    let tmp = std::env::temp_dir().join("agent-on-rails-cli-install");
    let archive = tmp.join("cli-main.tar.gz");
    let src = tmp.join("src");
    let _ = fs::remove_dir_all(&tmp);
    let _ = fs::create_dir_all(&src);

    let dl = download_cli_archive(&app, &archive);
    let mut log = String::new();
    if !dl.stdout.is_empty() {
        log.push_str(&dl.stdout);
        log.push('\n');
    }
    if !dl.ok || !archive.is_file() {
        emit_install(
            &app,
            "Installing CLI",
            40,
            "Archive download failed; trying git with a working git-remote-https…",
            false,
        );
        let mut result = install_from_spec(&app, &python, CLI_GIT);
        log.push_str(&result.stdout);
        result.stdout = log.trim().to_string();
        return finalize_install(&app, result);
    }

    emit_install(&app, "Extracting", 48, "Unpacking CLI source…", false);
    let mut tar = Command::new("tar");
    tar.args([
        "-xzf",
        archive.to_str().unwrap_or(""),
        "-C",
        src.to_str().unwrap_or(""),
        "--strip-components=1",
    ]);
    let extracted = stream_command(&app, tar, "Extracting", 48);
    log.push_str(&extracted.stdout);
    log.push('\n');
    if !extracted.ok {
        emit_install(&app, "Failed", 0, "Could not unpack the CLI archive.", true);
        return CommandResult {
            ok: false,
            stdout: log,
            stderr: extracted.stderr,
            hint: Some("Download succeeded but tar extract failed.".into()),
        };
    }

    emit_install(&app, "Installing CLI", 55, "Installing from local source (no git)…", false);
    let spec = src.display().to_string();
    let mut result = install_from_spec(&app, &python, &spec);
    log.push_str(&result.stdout);
    result.stdout = log.trim().to_string();
    finalize_install(&app, result)
}

fn install_from_spec(app: &AppHandle, python: &str, spec: &str) -> CommandResult {
    if which("pipx").is_some() {
        let mut cmd = Command::new("pipx");
        cmd.args(["install", "--force", spec]);
        return stream_command(app, cmd, "Installing CLI", 55);
    }
    if run_capture(python, &["-m", "pipx", "--version"]).ok {
        let mut cmd = Command::new(python);
        cmd.args(["-m", "pipx", "install", "--force", spec]);
        return stream_command(app, cmd, "Installing CLI", 55);
    }
    emit_install(
        app,
        "Installing CLI",
        58,
        "pipx not found — using python -m pip --user",
        false,
    );
    let mut cmd = Command::new(python);
    cmd.args(["-m", "pip", "install", "--user", "--upgrade", spec]);
    let mut r = stream_command(app, cmd, "Installing CLI", 60);
    if r.ok {
        r.hint = Some(
            "Installed with pip --user. If `aor` is missing from PATH, open a new terminal or add ~/.local/bin."
                .into(),
        );
    }
    r
}

fn finalize_install(app: &AppHandle, mut result: CommandResult) -> CommandResult {
    if result.ok {
        emit_install(app, "Verifying", 95, "Checking `aor --version`…", false);
        let ver = run_capture("aor", &["--version"]);
        if ver.ok {
            let v = if ver.stdout.is_empty() {
                ver.stderr
            } else {
                ver.stdout
            };
            result.stdout = format!("{}\n{}", result.stdout, v).trim().to_string();
            emit_install(app, "Installed", 100, &v, true);
        } else {
            result.hint = Some(
                "Package installed, but `aor` is not on PATH yet. Open a new terminal and retry, or run: python3 -m aor_cli"
                    .into(),
            );
            emit_install(
                app,
                "Installed",
                100,
                result.hint.as_deref().unwrap_or("Installed."),
                true,
            );
        }
        return result;
    }
    if result.hint.is_none() {
        result.hint = Some(
            "Install failed. The wizard downloads a GitHub tarball so a broken git-remote-https is not required."
                .into(),
        );
    }
    emit_install(app, "Failed", 0, result.hint.as_deref().unwrap_or("Install failed."), true);
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
    let mut r = run_aor(&args);
    if r.ok {
        r.hint = Some(format!(
            "Next: gather SurveyDesk-shaped specs from natural language, or skip to the terminal in `{path}`."
        ));
    } else if r.hint.is_none() && which("aor").is_none() {
        r.hint = Some("Install or upgrade the CLI, then retry.".into());
    }
    r
}

#[tauri::command]
fn gather_extract(path: String, requirements: String, stub: bool) -> GatherExtractResult {
    let path = path.trim().to_string();
    let requirements = requirements.trim().to_string();
    if path.is_empty() {
        return GatherExtractResult {
            ok: false,
            stdout: String::new(),
            stderr: "Choose a project folder on the previous step.".into(),
            hint: Some("Run aor init first, or pick an existing control-plane folder.".into()),
            outline: None,
            outline_path: None,
        };
    }
    if requirements.is_empty() {
        return GatherExtractResult {
            ok: false,
            stdout: String::new(),
            stderr: "Paste natural-language requirements first.".into(),
            hint: None,
            outline: None,
            outline_path: None,
        };
    }

    let req_dir = Path::new(&path).join(".aor").join("gather");
    if let Err(e) = fs::create_dir_all(&req_dir) {
        return GatherExtractResult {
            ok: false,
            stdout: String::new(),
            stderr: e.to_string(),
            hint: Some("Could not create .aor/gather in the project folder.".into()),
            outline: None,
            outline_path: None,
        };
    }
    let req_file = req_dir.join("source-requirements.txt");
    if let Err(e) = fs::write(&req_file, format!("{requirements}\n")) {
        return GatherExtractResult {
            ok: false,
            stdout: String::new(),
            stderr: e.to_string(),
            hint: None,
            outline: None,
            outline_path: None,
        };
    }

    let file_str = req_file.display().to_string();
    let modern = gather_extract_args(&path, &file_str, stub, true);
    let mut r = run_aor(&modern);
    if !r.ok && (unknown_cli_option(&r) || read_outline(&path).is_none()) {
        let compatible = gather_extract_args(&path, &file_str, stub, false);
        // Installed CLI may lack --outline-only; answer the confirm prompt with no
        // so it saves outline.json without writing specs.
        r = run_aor_with_stdin(&compatible, Some(b"n\n"));
    }
    let mut outline = read_outline(&path);
    if stub {
        if let Some(ref mut value) = outline {
            polish_stub_outline(value, &requirements);
            write_outline(&path, value);
        }
    }
    let outline_path = outline
        .as_ref()
        .map(|_| outline_json_path(&path).display().to_string());
    let ok = outline.is_some();
    GatherExtractResult {
        ok,
        stdout: if ok {
            format!(
                "Outline saved at {}",
                outline_path.as_deref().unwrap_or(".aor/gather/outline.json")
            )
        } else {
            r.stdout
        },
        stderr: r.stderr,
        hint: if ok {
            Some("Review the outline, then confirm to write SurveyDesk-shaped specs (drafts only).".into())
        } else {
            r.hint.or_else(|| {
                Some("Extract failed. Upgrade the CLI, then click Extract outline again.".into())
            })
        },
        outline,
        outline_path,
    }
}

#[tauri::command]
fn gather_saved_outline(path: String) -> Option<serde_json::Value> {
    read_outline(path.trim())
}

#[tauri::command]
fn gather_apply(path: String, force: bool) -> CommandResult {
    let path = path.trim().to_string();
    if path.is_empty() {
        return CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: "Choose a project folder on the previous step.".into(),
            hint: None,
        };
    }
    if read_outline(&path).is_none() {
        return CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: "No saved outline. Extract first.".into(),
            hint: Some("Paste requirements and extract an outline, then confirm.".into()),
        };
    }
    let args = gather_apply_args(&path, force);
    let mut r = run_aor(&args);
    if r.ok {
        r.hint = Some(
            "Drafts written. Specs are not APPROVED — review in the terminal, then approve."
                .into(),
        );
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

#[tauri::command]
fn open_folder(path: String) -> CommandResult {
    let path = path.trim();
    if path.is_empty() {
        return CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: "No project folder selected.".into(),
            hint: Some("Pick a project folder on the New project step first.".into()),
        };
    }
    let dir = PathBuf::from(path);
    if !dir.is_dir() {
        return CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: format!("Not a folder: {}", dir.display()),
            hint: Some("Choose an existing project folder.".into()),
        };
    }
    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = Command::new("explorer");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = Command::new("xdg-open");
    apply_env(&mut cmd);
    cmd.arg(&dir);
    match cmd.status() {
        Ok(st) if st.success() => CommandResult {
            ok: true,
            stdout: dir.display().to_string(),
            stderr: String::new(),
            hint: None,
        },
        Ok(st) => CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: format!("Open folder exited with {st}"),
            hint: None,
        },
        Err(e) => CommandResult {
            ok: false,
            stdout: String::new(),
            stderr: e.to_string(),
            hint: Some("Could not open the folder in the file manager.".into()),
        },
    }
}

#[cfg(target_os = "macos")]
fn install_macos_menu(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};

    let check = MenuItemBuilder::with_id("check-updates", "Check for Updates…").build(app)?;
    let app_submenu = SubmenuBuilder::new(app, "Agent On Rails")
        .item(&PredefinedMenuItem::about(app, None, None)?)
        .item(&check)
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .close_window()
        .build()?;
    let menu = MenuBuilder::new(app)
        .item(&app_submenu)
        .item(&edit)
        .item(&window)
        .build()?;
    app.set_menu(menu)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init());

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_plugin_sparkle_updater::init());
    }
    #[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                install_macos_menu(app.handle())?;
            }
            #[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
            {
                use tauri_plugin_updater::UpdaterExt;
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(updater) = handle.updater() {
                        if let Ok(Some(update)) = updater.check().await {
                            let _ = update.download_and_install(|_, _| {}, || {}).await;
                        }
                    }
                });
            }
            #[cfg(target_os = "macos")]
            {
                use tauri_plugin_sparkle_updater::SparkleUpdaterExt;
                if let Some(updater) = app.handle().sparkle_updater() {
                    let _ = updater.check_for_updates_in_background();
                }
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() != "check-updates" {
                return;
            }
            #[cfg(target_os = "macos")]
            {
                use tauri_plugin_sparkle_updater::SparkleUpdaterExt;
                if let Some(updater) = app.sparkle_updater() {
                    let _ = updater.check_for_updates();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            check_prerequisites,
            install_cli,
            init_project,
            gather_extract,
            gather_apply,
            gather_saved_outline,
            links,
            open_folder
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

    #[test]
    fn gather_extract_args_include_outline_only_and_stub() {
        let args = gather_extract_args("/tmp/demo", "/tmp/demo/.aor/gather/source-requirements.txt", true, true);
        assert!(args.contains(&"--outline-only".into()));
        assert!(args.contains(&"--json".into()));
        assert!(args.contains(&"--stub".into()));
        assert!(!args.iter().any(|a| a == "--yes"));
    }

    #[test]
    fn gather_extract_args_compatible_omits_outline_only() {
        let args = gather_extract_args("/tmp/demo", "/tmp/reqs.txt", true, false);
        assert!(!args.iter().any(|a| a == "--outline-only"));
        assert!(args.contains(&"--stub".into()));
    }

    #[test]
    fn gather_extract_args_omit_stub_when_using_llm() {
        let args = gather_extract_args("/tmp/demo", "/tmp/reqs.txt", false, true);
        assert!(!args.iter().any(|a| a == "--stub"));
    }

    #[test]
    fn gather_apply_args_confirm_yes() {
        let args = gather_apply_args("/tmp/demo", true);
        assert!(args.contains(&"--yes".into()));
        assert!(args.contains(&"--force".into()));
        assert_eq!(args[0], "gather");
        assert_eq!(args[1], "apply");
    }

    #[test]
    fn cli_archive_is_https_not_git() {
        assert!(CLI_ARCHIVE.starts_with("https://"));
        assert!(!CLI_ARCHIVE.contains("git+"));
    }

    #[test]
    fn parse_semver_and_newer() {
        assert_eq!(parse_semver("aor, version 0.3.0"), Some((0, 3, 0)));
        assert_eq!(parse_semver("0.4.1"), Some((0, 4, 1)));
        assert!(version_newer("0.4.0", "0.3.0"));
        assert!(!version_newer("0.3.0", "0.3.0"));
        assert!(!version_newer("0.2.9", "0.3.0"));
        assert_eq!(
            parse_init_version("__version__ = \"0.3.0\"\n"),
            Some("0.3.0".into())
        );
    }

    #[test]
    fn parse_curl_percent_from_progress_line() {
        assert_eq!(parse_percent("##############  45.0%"), Some(45));
        assert_eq!(parse_percent("  12.5%"), Some(12));
        assert_eq!(parse_percent("nope"), None);
    }

    #[test]
    fn stub_titles_do_not_end_on_articles() {
        let title = title_from_shall(
            "The system SHALL build a product called SurveyDesk: a local-first survey desk.",
        );
        assert!(title.to_lowercase().contains("local-first"));
        assert!(!title.to_lowercase().ends_with(" a"));
        let ipad = title_from_shall(
            "The system SHALL iPad and Android tablet use a left sidebar + detail.",
        );
        assert!(ipad.to_lowercase().contains("sidebar"));
        assert!(!ipad.to_lowercase().ends_with(" a"));
        assert_eq!(guess_product_name_from_text("called SurveyDesk: a"), Some("SurveyDesk".into()));
    }

    #[test]
    fn polish_does_not_rewrite_canonical_short_titles() {
        let mut outline = serde_json::json!({
            "product_name": "SurveyDesk",
            "requirements": [{
                "id": "SD-001",
                "title": "Product surfaces",
                "shall": "The system SHALL expose three cooperating surfaces: iOS and Android."
            }]
        });
        polish_stub_outline(&mut outline, "Build a product called SurveyDesk");
        assert_eq!(
            outline["requirements"][0]["title"].as_str().unwrap(),
            "Product surfaces"
        );
    }

    #[test]
    fn sibling_cli_launch_uses_local_checkout() {
        let root = sibling_cli_root().expect("agent-on-rails-cli should sit next to desktop");
        assert!(root.join("src/aor_cli/__main__.py").is_file());
        let (program, prefix, pythonpath) = aor_launch().expect("aor launch");
        assert!(
            program.contains("agent-on-rails-cli") || pythonpath.is_some() || which("aor").is_some(),
            "program={program:?} prefix={prefix:?}"
        );
        if let Some(pp) = pythonpath {
            assert!(pp.ends_with("src"));
        }
    }
}
