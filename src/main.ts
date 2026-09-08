import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";

type Prerequisites = {
  platform: string;
  pythonPath: string | null;
  pythonVersion: string | null;
  pythonOk: boolean;
  pipxAvailable: boolean;
  aorPath: string | null;
  aorVersion: string | null;
  notes: string[];
};

type CommandResult = {
  ok: boolean;
  stdout: string;
  stderr: string;
  hint: string | null;
};

type Links = {
  docs: string;
  walkthrough: string;
  cliRepo: string;
};

const STEPS = ["Welcome", "Prerequisites", "Install CLI", "New project", "Done"] as const;

let step = 0;
let prereq: Prerequisites | null = null;
let links: Links | null = null;
let projectPath = "";
let productName = "";
let forceInit = false;
let lastLog = "";
let busy = false;

const panel = () => document.querySelector("#panel") as HTMLElement;
const btnBack = () => document.querySelector("#btn-back") as HTMLButtonElement;
const btnNext = () => document.querySelector("#btn-next") as HTMLButtonElement;
const stepNav = () => document.querySelector("#step-nav") as HTMLElement;

function renderNav() {
  stepNav().innerHTML = STEPS.map((label, i) => {
    const cls = ["step-pill", i === step ? "active" : "", i < step ? "done" : ""]
      .filter(Boolean)
      .join(" ");
    return `<span class="${cls}">${i + 1}. ${label}</span>`;
  }).join("");
}

function setBusy(v: boolean) {
  busy = v;
  btnNext().disabled = v;
  btnBack().disabled = v;
}

function formatResult(r: CommandResult): string {
  const parts = [r.stdout, r.stderr, r.hint].filter(Boolean);
  return parts.join("\n\n");
}

async function refreshPrereq() {
  prereq = await invoke<Prerequisites>("check_prerequisites");
}

function renderWelcome() {
  panel().innerHTML = `
    <h1>Set up Agent On Rails</h1>
    <p class="lead">
      This wizard installs the <code>aor</code> CLI and bootstraps a docs-first
      project contract. Day-to-day work stays in the terminal UI.
    </p>
    <ul class="checklist">
      <li><span class="dot ok"></span><div><strong>Check</strong> Python 3.11+ and tools<div class="meta">macOS &amp; Windows</div></div></li>
      <li><span class="dot ok"></span><div><strong>Install</strong> the CLI via pipx (or pip fallback)</div></li>
      <li><span class="dot ok"></span><div><strong>Create</strong> a control-plane project with <code>aor init</code></div></li>
    </ul>
  `;
  btnNext().textContent = "Continue";
}

function renderPrereq() {
  const p = prereq!;
  const row = (ok: boolean, title: string, meta: string) => `
    <li>
      <span class="dot ${ok ? "ok" : "bad"}"></span>
      <div><strong>${title}</strong><div class="meta">${meta}</div></div>
    </li>`;
  panel().innerHTML = `
    <h1>Prerequisites</h1>
    <p class="lead">Detected on <strong>${p.platform}</strong>. Fix anything red, then continue.</p>
    <ul class="checklist">
      ${row(
        p.pythonOk,
        "Python 3.11+",
        p.pythonVersion
          ? `${p.pythonVersion}${p.pythonPath ? ` · ${p.pythonPath}` : ""}`
          : "Not found",
      )}
      ${row(
        p.pipxAvailable,
        "pipx",
        p.pipxAvailable ? "Available (preferred installer)" : "Missing — install will use pip --user",
      )}
      ${row(
        Boolean(p.aorPath),
        "aor CLI",
        p.aorPath
          ? `${p.aorVersion ?? "installed"} · ${p.aorPath}`
          : "Not installed yet (next step)",
      )}
    </ul>
    <div class="notes">${(p.notes || []).map((n) => `<div>${n}</div>`).join("")}</div>
    <div class="actions-inline">
      <button type="button" class="btn secondary" id="btn-recheck">Re-check</button>
    </div>
  `;
  document.querySelector("#btn-recheck")?.addEventListener("click", async () => {
    setBusy(true);
    await refreshPrereq();
    setBusy(false);
    render();
  });
  btnNext().textContent = p.pythonOk ? "Continue" : "I installed Python — re-check";
}

function renderInstall() {
  panel().innerHTML = `
    <h1>Install CLI</h1>
    <p class="lead">
      Installs or upgrades <code>aor</code> from
      <a class="linkish" href="${links?.cliRepo ?? "#"}" id="cli-repo-link">agent-on-rails-cli</a>.
    </p>
    <div class="actions-inline">
      <button type="button" class="btn secondary" id="btn-install">Install / upgrade aor</button>
    </div>
    <pre class="log" id="install-log">${escapeHtml(lastLog)}</pre>
  `;
  document.querySelector("#cli-repo-link")?.addEventListener("click", async (e) => {
    e.preventDefault();
    if (links?.cliRepo) await openUrl(links.cliRepo);
  });
  document.querySelector("#btn-install")?.addEventListener("click", async () => {
    setBusy(true);
    lastLog = "Installing…";
    renderInstall();
    try {
      const r = await invoke<CommandResult>("install_cli");
      lastLog = formatResult(r) || (r.ok ? "OK" : "Failed");
      await refreshPrereq();
    } catch (err) {
      lastLog = String(err);
    }
    setBusy(false);
    render();
  });
  btnNext().textContent = prereq?.aorPath ? "Continue" : "Skip for now";
}

function renderProject() {
  panel().innerHTML = `
    <h1>New project</h1>
    <p class="lead">Creates docs + specs only (control plane). No application code.</p>
    <div class="field">
      <label for="product-name">Product name</label>
      <input id="product-name" type="text" placeholder="FollowUp" value="${escapeAttr(productName)}" />
    </div>
    <div class="field">
      <label for="project-path">Project folder</label>
      <div class="row-inline">
        <input id="project-path" type="text" placeholder="/Users/you/projects/followup" value="${escapeAttr(projectPath)}" />
        <button type="button" class="btn secondary" id="btn-browse">Browse</button>
      </div>
    </div>
    <label class="check-row">
      <input id="force-init" type="checkbox" ${forceInit ? "checked" : ""} />
      Overwrite existing scaffold files (<code>--force</code>)
    </label>
    <div class="actions-inline">
      <button type="button" class="btn secondary" id="btn-init">Run aor init</button>
    </div>
    <pre class="log" id="init-log">${escapeHtml(lastLog)}</pre>
  `;

  const nameEl = document.querySelector("#product-name") as HTMLInputElement;
  const pathEl = document.querySelector("#project-path") as HTMLInputElement;
  const forceEl = document.querySelector("#force-init") as HTMLInputElement;
  nameEl.addEventListener("input", () => {
    productName = nameEl.value;
  });
  pathEl.addEventListener("input", () => {
    projectPath = pathEl.value;
  });
  forceEl.addEventListener("change", () => {
    forceInit = forceEl.checked;
  });

  document.querySelector("#btn-browse")?.addEventListener("click", async () => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Choose project folder",
    });
    if (typeof selected === "string") {
      projectPath = selected;
      pathEl.value = selected;
      if (!productName) {
        const base = selected.split(/[/\\]/).filter(Boolean).pop() || "";
        productName = base.replace(/-control-plane$/i, "");
        nameEl.value = productName;
      }
    }
  });

  document.querySelector("#btn-init")?.addEventListener("click", async () => {
    productName = nameEl.value.trim();
    projectPath = pathEl.value.trim();
    forceInit = forceEl.checked;
    setBusy(true);
    lastLog = "Running aor init…";
    renderProject();
    try {
      const r = await invoke<CommandResult>("init_project", {
        path: projectPath,
        name: productName,
        force: forceInit,
      });
      lastLog = formatResult(r) || (r.ok ? "OK" : "Failed");
      if (r.ok) {
        step = 4;
      }
    } catch (err) {
      lastLog = String(err);
    }
    setBusy(false);
    render();
  });

  btnNext().textContent = "Skip to done";
}

function renderDone() {
  panel().innerHTML = `
    <h1>You're ready</h1>
    <p class="lead">Open the terminal UI and follow the walkthrough for your first real case.</p>
    <ol class="done-list">
      <li>In a terminal: <code>cd ${escapeHtml(projectPath || "<project>")}</code> then <code>aor</code></li>
      <li>Create a spec, get a human approve it, then Plan → Run → Review</li>
      <li>Implement in your app repo with Cursor / Claude / Codex using the packaged prompt</li>
    </ol>
    <div class="actions-inline" style="margin-top:18px">
      <button type="button" class="btn secondary" id="btn-docs">Open docs</button>
      <button type="button" class="btn secondary" id="btn-walk">Open walkthrough</button>
      ${
        projectPath
          ? `<button type="button" class="btn secondary" id="btn-folder">Open project folder</button>`
          : ""
      }
    </div>
  `;
  document.querySelector("#btn-docs")?.addEventListener("click", async () => {
    if (links?.docs) await openUrl(links.docs);
  });
  document.querySelector("#btn-walk")?.addEventListener("click", async () => {
    if (links?.walkthrough) await openUrl(links.walkthrough);
  });
  document.querySelector("#btn-folder")?.addEventListener("click", async () => {
    if (projectPath) await openPath(projectPath);
  });
  btnNext().textContent = "Close";
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeAttr(s: string): string {
  return escapeHtml(s).replace(/'/g, "&#39;");
}

function render() {
  renderNav();
  btnBack().hidden = step === 0;
  if (step === 0) renderWelcome();
  else if (step === 1) renderPrereq();
  else if (step === 2) renderInstall();
  else if (step === 3) renderProject();
  else renderDone();
}

async function goNext() {
  if (busy) return;
  if (step === 4) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
    return;
  }
  if (step === 1 && prereq && !prereq.pythonOk) {
    setBusy(true);
    await refreshPrereq();
    setBusy(false);
    render();
    return;
  }
  if (step === 2 || step === 3) {
    lastLog = "";
  }
  step += 1;
  if (step === 1) {
    setBusy(true);
    await refreshPrereq();
    setBusy(false);
  }
  render();
}

function goBack() {
  if (busy || step === 0) return;
  lastLog = "";
  step -= 1;
  render();
}

window.addEventListener("DOMContentLoaded", async () => {
  try {
    links = await invoke<Links>("links");
  } catch {
    links = {
      docs: "https://agent-on-rails.suherman.net/docs/",
      walkthrough:
        "https://github.com/agent-on-rails/agent-on-rails-cli/blob/main/docs/walkthrough.md",
      cliRepo: "https://github.com/agent-on-rails/agent-on-rails-cli",
    };
  }
  btnBack().addEventListener("click", goBack);
  btnNext().addEventListener("click", () => void goNext());
  render();
});
