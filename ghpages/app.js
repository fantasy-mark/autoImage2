// autoimage — Pages edition.
//
// Talks to api.github.com and ghcr.io straight from the browser using a
// user-supplied PAT. No backend.

const TOKEN_KEY = "autoimage.ghToken";
const OWNER_KEY = "autoimage.owner";
const REPO_KEY = "autoimage.repo";

// ---- helpers ----
const $ = (s) => document.querySelector(s);
const $$ = (s) => Array.from(document.querySelectorAll(s));

function setText(id, msg, kind) {
  const el = $(id);
  if (!el) return;
  el.textContent = msg || "";
  el.className = "status" + (kind ? " " + kind : "");
}
function setEl(id, html, kind) {
  const el = $(id);
  if (!el) return;
  el.innerHTML = html;
  el.className = "status" + (kind ? " " + kind : "");
}
function getConfig() {
  return {
    token: localStorage.getItem(TOKEN_KEY) || "",
    owner: localStorage.getItem(OWNER_KEY) || "",
    repo: localStorage.getItem(REPO_KEY) || "",
  };
}
function saveToken(token) { localStorage.setItem(TOKEN_KEY, token); }
function saveOwner(owner) { localStorage.setItem(OWNER_KEY, owner); }
function saveRepo(repo) { localStorage.setItem(REPO_KEY, repo); }
function clearToken() { localStorage.removeItem(TOKEN_KEY); }

// b64-encode Unicode safely (works for any UTF-8 string)
function b64encode(str) {
  const bytes = new TextEncoder().encode(str);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

// ---- GitHub API wrapper ----
async function gh(path, { method = "GET", body = null, accept = "application/vnd.github+json" } = {}) {
  const { token, owner, repo } = getConfig();
  if (!token) throw new Error("missing token");
  const headers = {
    "Accept": accept,
    "Authorization": `Bearer ${token}`,
    "X-GitHub-Api-Version": "2022-11-28",
  };
  if (body !== null && !headers["Content-Type"]) headers["Content-Type"] = "application/json";
  const url = `https://api.github.com${path}`;
  const resp = await fetch(url, {
    method,
    headers,
    body: body !== null ? JSON.stringify(body) : undefined,
  });
  if (resp.status === 401) {
    clearToken();
    showGate("Token rejected by GitHub (401). Paste a new one.");
    throw new Error("401 Unauthorized");
  }
  return resp;
}

// ---- Dockerfile contents API ----
let _dockerfileSha = null;  // last-known SHA from the server

async function loadDockerfile() {
  setText("#df-status", "loading…");
  try {
    const { token, owner, repo } = getConfig();
    if (!owner || !repo) {
      setText("#df-status", "set owner + repo first");
      return;
    }
    const r = await gh(`/repos/${owner}/${repo}/contents/Dockerfile`);
    if (r.status === 404) {
      $("#df").value = "";
      _dockerfileSha = null;
      setText("#df-status", "no Dockerfile yet (will be created on Save)", "ok");
      scanAndRenderVariables();
      return;
    }
    if (!r.ok) {
      setText("#df-status", `load failed: ${r.status} ${(await r.text()).slice(0, 120)}`, "err");
      return;
    }
    const data = await r.json();
    _dockerfileSha = data.sha;
    // content is base64-encoded, may include \n line breaks
    const b64 = (data.content || "").replace(/\n/g, "");
    const text = decodeURIComponent(escape(atob(b64)));
    $("#df").value = text;
    setText("#df-status", `loaded (${data.size} bytes, sha ${data.sha.slice(0, 7)})`, "ok");
    scanAndRenderVariables();
  } catch (e) {
    setText("#df-status", `load failed: ${e.message}`, "err");
  }
}

async function saveDockerfile() {
  const raw = $("#df").value;
  const values = getVariableValues();
  const content = substituteVariables(raw, values);
  setText("#df-status", "saving…");
  try {
    const { owner, repo } = getConfig();
    const body = {
      message: `autoimage: update Dockerfile (${values.image || "?"}:${effectiveVersion()})`,
      content: b64encode(content),
    };
    if (_dockerfileSha) body.sha = _dockerfileSha;
    const r = await gh(`/repos/${owner}/${repo}/contents/Dockerfile`, { method: "PUT", body });
    if (r.status === 409) {
      setText("#df-status", "file changed on the server — please refresh first", "err");
      return;
    }
    if (!r.ok) {
      const txt = await r.text();
      setText("#df-status", `save failed: ${r.status} ${txt.slice(0, 200)}`, "err");
      return;
    }
    const data = await r.json();
    _dockerfileSha = data.content.sha;
    setText("#df-status", `saved ${data.content.sha.slice(0, 7)} (${data.content.size} bytes)`, "ok");
    $("#df").value = content;  // reflect substitutions back into the editor
  } catch (e) {
    setText("#df-status", `save failed: ${e.message}`, "err");
  }
}

// ---- variable detection + substitution ----
const VAR_RE = /\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}/g;

function scanVariables(content) {
  const out = new Map();
  if (!content) return [];
  let m;
  VAR_RE.lastIndex = 0;
  while ((m = VAR_RE.exec(content)) !== null) {
    const name = m[1];
    const def = m[2] !== undefined ? m[2] : null;
    if (!out.has(name)) out.set(name, { name, default: def });
  }
  return Array.from(out.values());
}

function getVariableValues() {
  const out = { image: $("#bf-image")?.value.trim() || "", version: $("#bf-version")?.value.trim() || "" };
  $$("#bv-list input.bv-value").forEach((inp) => {
    out[`var.${inp.dataset.name}`] = inp.value;
  });
  return out;
}

function substituteVariables(content, values) {
  return content.replace(VAR_RE, (full, name, def) => {
    const v = values && Object.prototype.hasOwnProperty.call(values, `var.${name}`) ? values[`var.${name}`] : "";
    if (v !== "" && v !== null && v !== undefined) return v;
    if (def !== undefined) return def;
    return full;
  });
}

const VERSION_DEFAULT = "latest";
function effectiveVersion() {
  return ($("#bf-version")?.value.trim() || VERSION_DEFAULT);
}

function renderVariables(vars) {
  const wrap = $("#bv-list");
  const prev = new Map();
  $$("#bv-list input.bv-value").forEach((inp) => {
    prev.set(inp.dataset.name, inp.value);
  });
  wrap.innerHTML = "";
  if (!vars.length) return;
  for (const v of vars) {
    const li = document.createElement("li");
    const name = document.createElement("span");
    name.className = "bv-name";
    name.textContent = "${" + v.name + "}";
    const input = document.createElement("input");
    input.className = "bv-value";
    input.placeholder = v.default !== null ? v.default : "(no default)";
    input.dataset.name = v.name;
    if (prev.has(v.name)) input.value = prev.get(v.name);
    li.appendChild(name);
    li.appendChild(input);
    if (v.default !== null) {
      const def = document.createElement("span");
      def.className = "bv-default";
      def.textContent = "default: " + v.default;
      li.appendChild(def);
    }
    wrap.appendChild(li);
  }
}

function scanAndRenderVariables() {
  renderVariables(scanVariables($("#df").value));
}

// ---- workflow dispatch ----
async function triggerBuild() {
  const image = $("#bf-image").value.trim();
  const version = effectiveVersion();
  setText("#bf-status", "saving Dockerfile…");
  try {
    // Save first so the dispatched commit reflects the current editor
    await saveDockerfile();
  } catch (e) {
    setText("#bf-status", `save failed — not dispatching: ${e.message}`, "err");
    return;
  }
  setText("#bf-status", "dispatching…");
  try {
    const { owner, repo } = getConfig();
    const r = await gh(`/repos/${owner}/${repo}/actions/workflows/build.yml/dispatches`, {
      method: "POST",
      body: {
        ref: "main",
        inputs: {
          repo: "ghcr.io",
          namespace: owner,
          image,
          version,
        },
      },
    });
    if (r.status === 204) {
      setText("#bf-status", `dispatched: build.yml (${image}:${version})`, "ok");
      updatePullCommand();
    } else if (r.status === 403) {
      const body = await r.text();
      setText("#bf-status", `403: PAT lacks actions:write — ${body.slice(0, 200)}`, "err");
    } else {
      const body = await r.text();
      setText("#bf-status", `dispatch failed: ${r.status} ${body.slice(0, 200)}`, "err");
    }
  } catch (e) {
    setText("#bf-status", `dispatch failed: ${e.message}`, "err");
  }
}

// ---- pull command ----
const PROXY_HOST = "proxy.vvvv.ee";
const NAMESPACE_DEFAULT = "fantasy-mark";

function updatePullCommand() {
  const platform = $("#bf-platform")?.value || "linux/amd64";
  const image = $("#bf-image")?.value.trim() || "<image>";
  const version = effectiveVersion();
  const ns = $("#cfg-owner")?.value.trim() || NAMESPACE_DEFAULT;
  const cmd = `podman pull --platform ${platform} ${PROXY_HOST}/ghcr.io/${ns}/${image}:${version}`;
  const el = $("#pull-cmd");
  if (el) el.textContent = cmd;
}

async function copyPullCommand() {
  const text = $("#pull-cmd")?.textContent || "";
  try {
    await navigator.clipboard.writeText(text);
    $("#pull-copy").textContent = "Copied!";
    setTimeout(() => ($("#pull-copy").textContent = "Copy"), 1500);
  } catch {
    // Fallback: select the text
    const el = $("#pull-cmd");
    const r = document.createRange();
    r.selectNodeContents(el);
    const sel = window.getSelection();
    sel.removeAllRanges();
    sel.addRange(r);
    document.execCommand("copy");
    sel.removeAllRanges();
  }
}

// ---- commit history ----
let _commits = [];
let _currentSha = null;

async function loadHistory() {
  setText("#hi-status", "loading…");
  try {
    const { owner, repo } = getConfig();
    const r = await gh(`/repos/${owner}/${repo}/commits?path=Dockerfile&per_page=20`);
    if (!r.ok) {
      setText("#hi-status", `load failed: ${r.status}`, "err");
      return;
    }
    _commits = await r.json();
    _currentSha = _dockerfileSha;
    renderHistory();
    setText("#hi-status", `loaded ${_commits.length} commit${_commits.length === 1 ? "" : "s"}`, "ok");
  } catch (e) {
    setText("#hi-status", `load failed: ${e.message}`, "err");
  }
}

function renderHistory() {
  const ul = $("#hi-list");
  ul.innerHTML = "";
  if (!_commits.length) {
    ul.innerHTML = '<li class="muted">(no Dockerfile commits yet)</li>';
    return;
  }
  for (const c of _commits) {
    const li = document.createElement("li");
    const sha = document.createElement("span");
    sha.className = "sha";
    sha.textContent = c.sha.slice(0, 7);
    const msg = document.createElement("span");
    msg.className = "msg";
    msg.textContent = c.commit.message.split("\n")[0];
    msg.title = `${c.commit.author?.name || "?"} — ${c.commit.author?.date || c.commit.committer?.date || ""}`;
    const view = document.createElement("button");
    view.type = "button";
    view.textContent = "view";
    view.onclick = () => viewRevision(c.sha);
    const diff = document.createElement("button");
    diff.type = "button";
    diff.textContent = "diff vs editor";
    diff.onclick = () => diffAgainstEditor(c.sha);
    li.appendChild(sha);
    li.appendChild(msg);
    li.appendChild(view);
    li.appendChild(diff);
    ul.appendChild(li);
  }
}

async function viewRevision(sha) {
  try {
    const { owner, repo } = getConfig();
    const r = await gh(`/repos/${owner}/${repo}/contents/Dockerfile?ref=${encodeURIComponent(sha)}`);
    if (!r.ok) {
      $("#hi-diff").textContent = `fetch failed: ${r.status}`;
      $("#hi-diff").classList.remove("hidden");
      return;
    }
    const data = await r.json();
    const b64 = (data.content || "").replace(/\n/g, "");
    const text = decodeURIComponent(escape(atob(b64)));
    $("#hi-diff").textContent = `--- ${sha.slice(0, 7)} ---\n${text}`;
    $("#hi-diff").classList.remove("hidden");
  } catch (e) {
    $("#hi-diff").textContent = `error: ${e.message}`;
    $("#hi-diff").classList.remove("hidden");
  }
}

async function diffAgainstEditor(sha) {
  try {
    const { owner, repo } = getConfig();
    const r = await gh(`/repos/${owner}/${repo}/contents/Dockerfile?ref=${encodeURIComponent(sha)}`);
    if (!r.ok) {
      $("#hi-diff").textContent = `fetch failed: ${r.status}`;
      $("#hi-diff").classList.remove("hidden");
      return;
    }
    const data = await r.json();
    const b64 = (data.content || "").replace(/\n/g, "");
    const oldText = decodeURIComponent(escape(atob(b64)));
    const newText = $("#df").value;
    $("#hi-diff").innerHTML = renderDiff(oldText, newText);
    $("#hi-diff").classList.remove("hidden");
  } catch (e) {
    $("#hi-diff").textContent = `error: ${e.message}`;
    $("#hi-diff").classList.remove("hidden");
  }
}

// Minimal Myers-style line diff (forward + good enough for ~1k-line Dockerfiles).
function renderDiff(a, b) {
  const al = a.split("\n");
  const bl = b.split("\n");
  const m = al.length, n = bl.length;
  // LCS dp (O(mn) memory; OK for small files)
  const dp = Array.from({ length: m + 1 }, () => new Uint32Array(n + 1));
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      dp[i][j] = al[i - 1] === bl[j - 1] ? dp[i - 1][j - 1] + 1 : Math.max(dp[i - 1][j], dp[i][j - 1]);
    }
  }
  const out = [];
  let i = m, j = n;
  while (i > 0 && j > 0) {
    if (al[i - 1] === bl[j - 1]) { out.unshift([" ", al[i - 1]]); i--; j--; }
    else if (dp[i - 1][j] >= dp[i][j - 1]) { out.unshift(["-", al[i - 1]]); i--; }
    else { out.unshift(["+", bl[j - 1]]); j--; }
  }
  while (i > 0) { out.unshift(["-", al[--i]]); }
  while (j > 0) { out.unshift(["+", bl[--j]]); }
  const html = out.map(([op, line]) => {
    const cls = op === "+" ? "diff-add" : op === "-" ? "diff-del" : "";
    const safe = line.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    return `<span class="${cls}">${op} ${safe}</span>`;
  }).join("\n");
  return html || "(no differences)";
}

// ---- gate / app visibility ----
function showGate(errMsg) {
  $("#gate").classList.remove("hidden");
  $("#app").classList.add("hidden");
  $("#auth-warning").classList.add("hidden");
  if (errMsg) {
    $("#gate-err").textContent = errMsg;
    $("#gate-err").classList.remove("hidden");
  } else {
    $("#gate-err").classList.add("hidden");
  }
}
function showApp() {
  $("#gate").classList.add("hidden");
  $("#app").classList.remove("hidden");
  $("#auth-warning").classList.remove("hidden");
}

function applyConfigToInputs() {
  const { token, owner, repo } = getConfig();
  $("#cfg-owner").value = owner;
  $("#cfg-repo").value = repo;
  $("#gate-owner").value = owner;
  $("#gate-repo").value = repo;
  // Don't put the token in plain input; just mark that we have one
  $("#cfg-token").value = "";
}

async function init() {
  const { token, owner, repo } = getConfig();
  applyConfigToInputs();
  if (!token) {
    showGate();
    return;
  }
  showApp();
  // wire events first so user sees feedback even if loads fail
  wireEvents();
  // initial loads
  try { await loadDockerfile(); } catch {}
  try { await loadHistory(); } catch {}
}

function wireEvents() {
  $("#cfg-save").onclick = () => {
    const tok = $("#cfg-token").value.trim();
    if (!tok) return;
    saveToken(tok);
    $("#cfg-save-msg").textContent = "saved";
    setTimeout(() => ($("#cfg-save-msg").textContent = ""), 1500);
  };
  $("#cfg-owner").addEventListener("input", (e) => {
    saveOwner(e.target.value.trim());
    updatePullCommand();
  });
  $("#cfg-repo").addEventListener("input", (e) => {
    saveRepo(e.target.value.trim());
    updatePullCommand();
  });
  $("#cfg-token-toggle").onclick = () => {
    $("#token-form").classList.toggle("hidden");
  };

  $("#df-refresh").onclick = loadDockerfile;
  $("#df-save").onclick = saveDockerfile;
  $("#df").addEventListener("input", scanAndRenderVariables);

  $("#bv-defaults").onclick = () => {
    $$("#bv-list input.bv-value").forEach((inp) => {
      inp.value = inp.placeholder && inp.placeholder !== "(no default)" ? inp.placeholder : "";
    });
  };

  $("#bf-build").onclick = triggerBuild;
  $("#bf-image").addEventListener("input", updatePullCommand);
  $("#bf-version").addEventListener("input", updatePullCommand);
  $("#bf-platform").addEventListener("change", updatePullCommand);

  $("#pull-copy").onclick = copyPullCommand;

  $("#hi-refresh").onclick = loadHistory;
}

function wireGate() {
  $("#gate-save").onclick = async () => {
    const tok = $("#gate-token").value.trim();
    const owner = $("#gate-owner").value.trim();
    const repo = $("#gate-repo").value.trim();
    if (!tok) { showGate("Token is required."); return; }
    if (!owner || !repo) { showGate("Owner and repo are required."); return; }
    saveToken(tok);
    saveOwner(owner);
    saveRepo(repo);
    $("#gate-err").classList.add("hidden");
    // quick auth probe: fetch the repo
    try {
      const r = await gh(`/repos/${owner}/${repo}`);
      if (!r.ok && r.status !== 304) {
        showGate(`Cannot read repo (${r.status}): ${(await r.text()).slice(0, 120)}`);
        return;
      }
    } catch (e) {
      showGate(`Auth probe failed: ${e.message}`);
      return;
    }
    applyConfigToInputs();
    showApp();
    wireEvents();
    try { await loadDockerfile(); } catch {}
    try { await loadHistory(); } catch {}
    updatePullCommand();
  };
}

window.addEventListener("DOMContentLoaded", () => {
  wireGate();
  init();
});
