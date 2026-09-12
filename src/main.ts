import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";

type Prerequisites = {
  platform: string;
  pythonPath: string | null;
  pythonVersion: string | null;
  pythonOk: boolean;
  pipxAvailable: boolean;
  aorPath: string | null;
  aorVersion: string | null;
  aorLatestVersion: string | null;
  aorUpdateAvailable: boolean;
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

type GatherExtractResult = {
  ok: boolean;
  stdout: string;
  stderr: string;
  hint: string | null;
  outline: GatherOutline | null;
  outlinePath: string | null;
};

type GatherOutline = {
  product_name?: string;
  tagline?: string;
  vision?: string;
  requirements?: Array<{ id?: string; title?: string; shall?: string }>;
  domains?: Array<{ name?: string }>;
  adrs?: Array<{ id?: string; title?: string }>;
};

type InstallProgress = {
  stage: string;
  percent: number;
  line: string;
  done: boolean;
};

const STEPS = ["Welcome", "Prerequisites", "Install CLI", "New project", "Gather specs", "Done"] as const;
const STEP_INSTALL = 2;
const STEP_PROJECT = 3;
const STEP_GATHER = 4;
const STEP_DONE = 5;

let step = 0;
let prereq: Prerequisites | null = null;
let links: Links | null = null;
let projectPath = "";
let productName = "";
let forceInit = false;
let lastLog = "";
let busy = false;
let requirementsText = "";
let useStubExtract = true;
let forceGather = true;
let outline: GatherOutline | null = null;
let gatherWrote = false;

const panel = () => document.querySelector("#panel") as HTMLElement;
const btnBack = () => document.querySelector("#btn-back") as HTMLButtonElement;
const btnNext = () => document.querySelector("#btn-next") as HTMLButtonElement;
const stepNav = () => document.querySelector("#step-nav") as HTMLElement;

function needsCliInstallStep(): boolean {
  if (!prereq) return false;
  return !prereq.aorPath || prereq.aorUpdateAvailable;
}

function renderNav() {
  const pills = STEPS.map((label, i) => ({ label, i })).filter(
    ({ i }) => i !== STEP_INSTALL || needsCliInstallStep() || step === STEP_INSTALL,
  );
  stepNav().innerHTML = pills
    .map(({ label, i }, vis) => {
      const cls = ["step-pill", i === step ? "active" : "", i < step ? "done" : ""]
        .filter(Boolean)
        .join(" ");
      return `<span class="${cls}">${vis + 1}. ${label}</span>`;
    })
    .join("");
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

function installModal() {
  return document.querySelector("#install-modal") as HTMLElement;
}

function applyInstallProgress(p: InstallProgress) {
  const stage = document.querySelector("#install-stage") as HTMLElement | null;
  const bar = document.querySelector("#install-bar") as HTMLElement | null;
  const meter = document.querySelector("#install-progressbar") as HTMLElement | null;
  const stream = document.querySelector("#install-stream") as HTMLElement | null;
  const close = document.querySelector("#install-close") as HTMLButtonElement | null;
  if (stage) stage.textContent = p.stage;
  if (bar) bar.style.width = `${Math.max(0, Math.min(100, p.percent))}%`;
  meter?.setAttribute("aria-valuenow", String(p.percent));
  if (stream && p.line) {
    stream.textContent = `${stream.textContent || ""}${p.line}\n`;
    stream.scrollTop = stream.scrollHeight;
  }
  if (p.done && close) {
    close.disabled = false;
    close.textContent = p.stage === "Failed" ? "Close" : "Done";
  }
}

function showInstallModal() {
  const modal = installModal();
  const stream = document.querySelector("#install-stream") as HTMLElement | null;
  const close = document.querySelector("#install-close") as HTMLButtonElement | null;
  const bar = document.querySelector("#install-bar") as HTMLElement | null;
  const stage = document.querySelector("#install-stage") as HTMLElement | null;
  if (stream) stream.textContent = "";
  if (bar) bar.style.width = "4%";
  if (stage) stage.textContent = "Starting…";
  if (close) {
    close.disabled = true;
    close.textContent = "Done";
  }
  modal.hidden = false;
}

function hideInstallModal() {
  installModal().hidden = true;
}

async function installAor() {
  if (busy || !installModal().hidden) return;
  lastLog = "Installing aor…";
  setBusy(true);
  showInstallModal();
  let unlisten: UnlistenFn | undefined;
  try {
    unlisten = await listen<InstallProgress>("install-progress", (event) => {
      applyInstallProgress(event.payload);
    });
    const r = await invoke<CommandResult>("install_cli");
    lastLog = formatResult(r) || (r.ok ? "OK" : "Failed");
    applyInstallProgress({
      stage: r.ok ? "Installed" : "Failed",
      percent: r.ok ? 100 : 0,
      line: "",
      done: true,
    });
    await refreshPrereq();
  } catch (err) {
    lastLog = String(err);
    applyInstallProgress({
      stage: "Failed",
      percent: 0,
      line: lastLog,
      done: true,
    });
  } finally {
    unlisten?.();
    setBusy(false);
    render();
    installModal().hidden = false;
  }
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
      <li><span class="dot ok"></span><div><strong>Gather</strong> SurveyDesk-shaped specs from natural language (optional)</div></li>
    </ul>
  `;
  btnNext().textContent = "Continue";
}

function renderPrereq() {
  const p = prereq!;
  const aorInstalled = Boolean(p.aorPath);
  const showInstall = !aorInstalled;
  const showUpgrade = aorInstalled && p.aorUpdateAvailable;
  const aorMeta = aorInstalled
    ? `${p.aorVersion ?? "installed"}${
        p.aorUpdateAvailable && p.aorLatestVersion
          ? ` · update ${escapeHtml(p.aorLatestVersion)} available`
          : p.aorLatestVersion
            ? " · up to date"
            : ""
      }${p.aorPath ? ` · ${p.aorPath}` : ""}`
    : "Not installed";
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
      ${row(aorInstalled, "aor CLI", aorMeta)}
    </ul>
    <div class="notes">${(p.notes || []).map((n) => `<div>${n}</div>`).join("")}</div>
    <div class="actions-inline">
      ${
        showInstall
          ? `<button type="button" class="btn primary" id="btn-install-prereq" ${p.pythonOk && !busy ? "" : "disabled"}>Install aor</button>`
          : ""
      }
      ${
        showUpgrade
          ? `<button type="button" class="btn primary" id="btn-install-prereq" ${p.pythonOk && !busy ? "" : "disabled"}>Upgrade aor</button>`
          : ""
      }
      <button type="button" class="btn secondary" id="btn-recheck" ${busy ? "disabled" : ""}>Re-check</button>
    </div>
    <pre class="log" id="prereq-log">${escapeHtml(lastLog)}</pre>
  `;
  document.querySelector("#btn-install-prereq")?.addEventListener("click", () => {
    void installAor();
  });
  document.querySelector("#btn-recheck")?.addEventListener("click", async () => {
    setBusy(true);
    await refreshPrereq();
    setBusy(false);
    render();
  });
  btnNext().textContent = p.pythonOk ? "Continue" : "I installed Python — re-check";
}

function renderInstall() {
  const upgrading = Boolean(prereq?.aorPath);
  panel().innerHTML = `
    <h1>Install CLI</h1>
    <p class="lead">
      ${upgrading ? "Upgrades" : "Installs"} <code>aor</code> from
      <a class="linkish" href="${links?.cliRepo ?? "#"}" id="cli-repo-link">agent-on-rails-cli</a>.
    </p>
    <div class="actions-inline">
      <button type="button" class="btn secondary" id="btn-install">${upgrading ? "Upgrade aor" : "Install aor"}</button>
    </div>
    <pre class="log" id="install-log">${escapeHtml(lastLog)}</pre>
  `;
  document.querySelector("#cli-repo-link")?.addEventListener("click", async (e) => {
    e.preventDefault();
    if (links?.cliRepo) await openUrl(links.cliRepo);
  });
  document.querySelector("#btn-install")?.addEventListener("click", () => {
    void installAor();
  });
  btnNext().textContent = prereq?.aorPath ? "Continue" : "Skip for now";
}

function productNameFromFolder(path: string): string {
  const base = (path.split(/[/\\]/).filter(Boolean).pop() || "").replace(
    /-control-plane$/i,
    "",
  );
  if (!base) return "";
  return base
    .replace(/([a-z\d])([A-Z])/g, "$1 $2")
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
    .join(" ");
}

function projectFieldsReady(): boolean {
  return Boolean(productName.trim() && projectPath.trim());
}

function renderProject() {
  if (projectPath && !productName) {
    productName = productNameFromFolder(projectPath);
  }
  panel().innerHTML = `
    <h1>New project</h1>
    <p class="lead">Creates docs + specs only (control plane). No application code.</p>
    <div class="field">
      <label for="project-path">Project folder</label>
      <div class="row-inline">
        <input id="project-path" type="text" placeholder="/Users/you/projects/followup" value="${escapeAttr(projectPath)}" />
        <button type="button" class="btn secondary" id="btn-browse">Browse</button>
      </div>
    </div>
    <div class="field">
      <label for="product-name">Product name</label>
      <input id="product-name" type="text" placeholder="FollowUp" value="${escapeAttr(productName)}" />
    </div>
    <label class="check-row">
      <input id="force-init" type="checkbox" ${forceInit ? "checked" : ""} />
      Overwrite existing scaffold files (<code>--force</code>)
    </label>
    <pre class="log" id="init-log">${escapeHtml(lastLog)}</pre>
  `;

  const nameEl = document.querySelector("#product-name") as HTMLInputElement;
  const pathEl = document.querySelector("#project-path") as HTMLInputElement;
  const forceEl = document.querySelector("#force-init") as HTMLInputElement;
  const syncProjectActions = () => {
    btnNext().disabled = busy || !projectFieldsReady();
  };
  nameEl.addEventListener("input", () => {
    productName = nameEl.value;
    syncProjectActions();
  });
  pathEl.addEventListener("input", () => {
    projectPath = pathEl.value;
    productName = productNameFromFolder(projectPath);
    nameEl.value = productName;
    syncProjectActions();
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
      productName = productNameFromFolder(selected);
      nameEl.value = productName;
      syncProjectActions();
    }
  });

  btnNext().textContent = "Continue to gather";
  btnNext().disabled = busy || !projectFieldsReady();
}

async function runInitProject(): Promise<boolean> {
  const nameEl = document.querySelector("#product-name") as HTMLInputElement | null;
  const pathEl = document.querySelector("#project-path") as HTMLInputElement | null;
  const forceEl = document.querySelector("#force-init") as HTMLInputElement | null;
  if (nameEl) productName = nameEl.value.trim();
  if (pathEl) projectPath = pathEl.value.trim();
  if (forceEl) forceInit = forceEl.checked;
  if (!projectFieldsReady()) return false;
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
      lastLog = "";
      outline = null;
      requirementsText = "";
      gatherWrote = false;
      return true;
    }
    return false;
  } catch (err) {
    lastLog = String(err);
    return false;
  } finally {
    setBusy(false);
  }
}

function outlineHtml(o: GatherOutline): string {
  const reqs = (o.requirements || [])
    .map((r) => {
      const id = escapeHtml(r.id || "");
      const title = escapeHtml(r.title || "");
      const shall = escapeHtml(r.shall || "");
      return `<li>
        <span class="req-id">${id}</span>
        <div class="req-body">
          <strong class="req-title">${title}</strong>
          ${shall ? `<p class="req-shall">${shall}</p>` : ""}
        </div>
      </li>`;
    })
    .join("");
  const domains = (o.domains || []).map((d) => d.name).filter(Boolean).join(", ") || "—";
  const adrs = (o.adrs || [])
    .map((a) => a.id || a.title)
    .filter(Boolean)
    .join(", ") || "—";
  return `
    <div class="outline-card">
      <h2>${escapeHtml(o.product_name || "Outline")}</h2>
      <p class="outline-tagline">${escapeHtml(o.tagline || o.vision || "")}</p>
      <ul class="outline-reqs">${reqs || "<li>No requirements in outline</li>"}</ul>
      <p class="meta" style="margin-top:10px">Domains: ${escapeHtml(domains)} · ADRs: ${escapeHtml(adrs)}</p>
    </div>`;
}

function renderGather() {
  const hasPath = Boolean(projectPath);
  const hasRequirements = Boolean(requirementsText.trim());
  const showOutline = Boolean(outline && hasRequirements);
  panel().innerHTML = `
    <h1>Gather specs</h1>
    <p class="lead">
      Paste product requirements. The wizard calls <code>aor gather</code> (AOR-010),
      shows a SurveyDesk-shaped outline, and writes drafts only after you confirm.
      Specs stay unapproved. Install or upgrade the CLI on Prerequisites if <code>aor gather</code> is missing.
    </p>
    ${
      hasPath
        ? `<p class="meta">Project: <code>${escapeHtml(projectPath)}</code></p>`
        : `<div class="notes">Pick a project folder on the previous step first.</div>`
    }
    <div class="field">
      <label for="requirements">Natural-language requirements</label>
      <textarea id="requirements" rows="7" placeholder="Build a local survey desk: native mobile operators, anonymous public web, SQLite API, instant results…">${escapeHtml(requirementsText)}</textarea>
    </div>
    <label class="check-row">
      <input id="use-stub" type="checkbox" ${useStubExtract ? "checked" : ""} />
      Offline stub extract (no LLM). Uncheck to use <code>AOR_LLM_API_KEY</code> / llm.yaml on this machine.
    </label>
    <label class="check-row">
      <input id="force-gather" type="checkbox" ${forceGather ? "checked" : ""} />
      Overwrite existing generated spec files (<code>--force</code>)
    </label>
    <div class="actions-inline">
      <button type="button" class="btn secondary" id="btn-extract" ${hasPath && hasRequirements && !busy ? "" : "disabled"}>Extract outline</button>
    </div>
    ${
      showOutline
        ? ""
        : `<div class="notes">Confirm &amp; write drafts is locked until you paste requirements, click <strong>Extract outline</strong>, and an outline appears.</div>`
    }
    ${showOutline ? outlineHtml(outline!) : ""}
    <pre class="log" id="gather-log">${escapeHtml(lastLog)}</pre>
  `;

  const reqEl = document.querySelector("#requirements") as HTMLTextAreaElement | null;
  const stubEl = document.querySelector("#use-stub") as HTMLInputElement | null;
  const forceEl = document.querySelector("#force-gather") as HTMLInputElement | null;
  const extractEl = document.querySelector("#btn-extract") as HTMLButtonElement | null;
  reqEl?.addEventListener("input", () => {
    requirementsText = reqEl.value;
    if (extractEl) extractEl.disabled = busy || !hasPath || !requirementsText.trim();
    btnNext().disabled = busy || !outline || !requirementsText.trim();
    const card = document.querySelector(".outline-card") as HTMLElement | null;
    if (card) card.hidden = !requirementsText.trim();
  });
  stubEl?.addEventListener("change", () => {
    useStubExtract = stubEl.checked;
  });
  forceEl?.addEventListener("change", () => {
    forceGather = forceEl.checked;
  });

  document.querySelector("#btn-extract")?.addEventListener("click", async () => {
    requirementsText = reqEl?.value.trim() ?? "";
    useStubExtract = stubEl?.checked ?? true;
    setBusy(true);
    lastLog = "Extracting outline…";
    outline = null;
    renderGather();
    try {
      const r = await invoke<GatherExtractResult>("gather_extract", {
        path: projectPath,
        requirements: requirementsText,
        stub: useStubExtract,
      });
      outline = r.outline;
      lastLog = formatResult(r) || (r.ok ? "Outline ready — Confirm & write drafts is unlocked." : "Failed");
      if (!outline) {
        lastLog = `${lastLog}\n\nConfirm stays disabled until extract saves an outline.`;
      }
    } catch (err) {
      lastLog = String(err);
    }
    setBusy(false);
    render();
  });

  btnNext().textContent = "Confirm & write drafts";
  btnNext().disabled = busy || !showOutline;
}

async function applyGather() {
  if (busy || !outline || !projectPath) return;
  const forceEl = document.querySelector("#force-gather") as HTMLInputElement | null;
  forceGather = forceEl?.checked ?? forceGather;
  setBusy(true);
  lastLog = "Writing SurveyDesk-shaped specs…";
  renderGather();
  try {
    const r = await invoke<CommandResult>("gather_apply", {
      path: projectPath,
      force: forceGather,
    });
    lastLog = formatResult(r) || (r.ok ? "OK" : "Failed");
    if (r.ok) {
      gatherWrote = true;
      step = STEP_DONE;
    }
  } catch (err) {
    lastLog = String(err);
  }
  setBusy(false);
  render();
}

function renderDone() {
  panel().innerHTML = `
    <h1>You're ready</h1>
    <p class="lead">Open the terminal UI and follow the walkthrough for your first real case.</p>
    <ol class="done-list">
      <li>In a terminal: <code>cd ${escapeHtml(projectPath || "<project>")}</code> then <code>aor</code></li>
      ${
        gatherWrote
          ? `<li>Review the SurveyDesk-shaped drafts under <code>specs/</code>, then human-approve before agents implement</li>`
          : `<li>Optional: <code>aor gather run "…requirements…"</code> for a SurveyDesk-shaped pack, then human-approve</li>`
      }
      <li>Plan → Run → Review. Implement in your app repo with Cursor / Claude / Codex using the packaged prompt</li>
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
    <div class="notes" id="folder-error" hidden></div>
  `;
  document.querySelector("#btn-docs")?.addEventListener("click", async () => {
    if (links?.docs) await openUrl(links.docs);
  });
  document.querySelector("#btn-walk")?.addEventListener("click", async () => {
    if (links?.walkthrough) await openUrl(links.walkthrough);
  });
  document.querySelector("#btn-folder")?.addEventListener("click", async () => {
    if (!projectPath) return;
    const errEl = document.querySelector("#folder-error") as HTMLElement | null;
    try {
      const r = await invoke<CommandResult>("open_folder", { path: projectPath });
      if (!r.ok && errEl) {
        errEl.hidden = false;
        errEl.textContent = formatResult(r) || "Could not open the project folder.";
      }
    } catch (err) {
      if (errEl) {
        errEl.hidden = false;
        errEl.textContent = String(err);
      }
    }
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
  if (step === STEP_INSTALL && !needsCliInstallStep()) {
    step = STEP_PROJECT;
  }
  renderNav();
  btnBack().hidden = step === 0;
  if (step === 0) renderWelcome();
  else if (step === 1) renderPrereq();
  else if (step === STEP_INSTALL) renderInstall();
  else if (step === STEP_PROJECT) renderProject();
  else if (step === STEP_GATHER) renderGather();
  else renderDone();
  if (step === STEP_PROJECT) {
    btnNext().disabled = busy || !projectFieldsReady();
  } else if (step !== STEP_GATHER) {
    btnNext().disabled = busy;
  }
}

async function goNext() {
  if (busy) return;
  if (step === STEP_DONE) {
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
  if (step === 1) {
    setBusy(true);
    await refreshPrereq();
    setBusy(false);
    lastLog = "";
    step = needsCliInstallStep() ? STEP_INSTALL : STEP_PROJECT;
    render();
    return;
  }
  if (step === STEP_PROJECT) {
    if (!projectFieldsReady()) return;
    const ok = await runInitProject();
    if (!ok) {
      render();
      return;
    }
    step = STEP_GATHER;
    render();
    return;
  }
  if (step === STEP_GATHER) {
    await applyGather();
    return;
  }
  if (step === STEP_INSTALL) {
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
  if (step === STEP_PROJECT && !needsCliInstallStep()) {
    step = 1;
  } else {
    step -= 1;
  }
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
  document.querySelector("#install-close")?.addEventListener("click", hideInstallModal);
  render();
});
