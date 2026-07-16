// minimal vanilla-JS client for autoimage
const $ = (s) => document.querySelector(s);
const $$ = (s) => Array.from(document.querySelectorAll(s));
const setStatus = (msg, kind) => {
  const el = $("#df-status");
  el.textContent = msg || "";
  el.className = "status" + (kind ? " " + kind : "");
};

async function call(method, path, body) {
  const opts = { method, headers: {} };
  if (body !== undefined) {
    opts.headers["content-type"] = "application/json";
    opts.body = JSON.stringify(body);
  }
  const r = await fetch(path, opts);
  const text = await r.text();
  let parsed;
  try { parsed = text ? JSON.parse(text) : null; } catch { parsed = text; }
  if (!r.ok) {
    const err = new Error((parsed && parsed.error) || `HTTP ${r.status}`);
    err.status = r.status;
    err.body = parsed;
    throw err;
  }
  return parsed;
}

async function loadDockerfile() {
  try {
    const data = await call("GET", "/api/dockerfile");
    $("#df").value = data.content;
    setStatus(`loaded (${data.size} bytes, updated ${data.updated_at})`, "ok");
    scanAndRenderVariables();
  } catch (e) {
    if (e.status === 404) {
      $("#df").value = "";
      setStatus("no Dockerfile yet", "");
    } else {
      setStatus(`load failed: ${e.message}`, "err");
    }
  }
}

// ---- build variable scanning + substitution ----

// Match ${VAR} or ${VAR:-default}. Skips escaped \${ ... }.
const VAR_RE = /\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}/g;

function scanVariables(content) {
  const out = new Map(); // name -> { name, default }
  if (!content) return [];
  let m;
  // Reset regex state
  VAR_RE.lastIndex = 0;
  while ((m = VAR_RE.exec(content)) !== null) {
    const name = m[1];
    const def = m[2] !== undefined ? m[2] : null;
    // Preserve the first-seen default (or null if no `:-` was given)
    if (!out.has(name)) out.set(name, { name, default: def });
  }
  return Array.from(out.values());
}

function getVariableValues() {
  // Read the current input values for each variable row.
  const out = {};
  $$("#bv-list input.bv-value").forEach((inp) => {
    out[inp.dataset.name] = inp.value;
  });
  return out;
}

function substituteVariables(content, values) {
  // Replace ${VAR} / ${VAR:-default} with the provided value.
  // - if user provided a non-empty value, use it
  // - else if there's a default, use the default
  // - else leave as `${VAR}` so Docker fails loudly (matches behaviour of
  //   `docker build` without `--build-arg VAR=...`).
  return content.replace(VAR_RE, (full, name, def) => {
    const v = values && Object.prototype.hasOwnProperty.call(values, name) ? values[name] : "";
    if (v !== "" && v !== null && v !== undefined) return v;
    if (def !== undefined) return def;
    return full;
  });
}

function renderVariables(vars) {
  const wrap = $("#bv-wrap");
  const list = $("#bv-list");
  // capture current values + placeholder defaults before wiping the DOM
  const prev = new Map();
  $$("#bv-list input.bv-value").forEach((inp) => {
    prev.set(inp.dataset.name, { value: inp.value, default: inp.dataset.def || "" });
  });
  list.innerHTML = "";
  if (!vars.length) {
    wrap.classList.add("hidden");
    return;
  }
  wrap.classList.remove("hidden");
  for (const v of vars) {
    const li = document.createElement("li");
    const name = document.createElement("span");
    name.className = "bv-name";
    name.textContent = "${" + v.name + "}";
    const input = document.createElement("input");
    input.className = "bv-value";
    input.placeholder = v.default !== null ? v.default : "(no default)";
    input.dataset.name = v.name;
    input.dataset.def = v.default !== null ? v.default : "";
    // restore a previously-entered value if the variable name still exists
    if (prev.has(v.name)) input.value = prev.get(v.name).value;
    const def = document.createElement("span");
    def.className = "bv-default";
    if (v.default !== null) def.textContent = "default: " + v.default;
    li.appendChild(name);
    li.appendChild(input);
    li.appendChild(def);
    list.appendChild(li);
  }
}

function scanAndRenderVariables() {
  const vars = scanVariables($("#df").value);
  renderVariables(vars);
}

async function saveDockerfile(opts) {
  const silent = opts && opts.silent;
  const raw = $("#df").value;
  const values = getVariableValues();
  const content = substituteVariables(raw, values);
  if (!silent) setStatus("saving...");
  try {
    const data = await call("PUT", "/api/dockerfile", { content });
    const subNote = values && Object.keys(values).some((k) => values[k])
      ? ` (substituted ${Object.keys(values).filter((k) => values[k]).length} variable${Object.keys(values).filter((k) => values[k]).length === 1 ? "" : "s"})`
      : "";
    if (!silent) setStatus(`saved (${data.size} bytes, backup ${data.backup})${subNote}`, "ok");
    await loadBackups();
    return true;
  } catch (e) {
    if (!silent) setStatus(`save failed: ${e.message}`, "err");
    return false;
  }
}

async function loadBackups() {
  const list = $("#bf-list");
  list.innerHTML = "";
  try {
    const { backups } = await call("GET", "/api/dockerfile/backups");
    if (!backups.length) {
      list.innerHTML = '<li><span>(no backups)</span></li>';
      return;
    }
    for (const b of backups) {
      const li = document.createElement("li");
      const a = document.createElement("a");
      a.textContent = b.name;
      a.onclick = () => viewBackup(b.name);
      const meta = document.createElement("span");
      meta.textContent = `${b.size}B · ${b.created_at}`;
      li.appendChild(a);
      li.appendChild(meta);
      list.appendChild(li);
    }
  } catch (e) {
    list.innerHTML = `<li>error: ${e.message}</li>`;
  }
}

async function viewBackup(name) {
  try {
    const { content } = await call("GET", `/api/dockerfile/backups/${encodeURIComponent(name)}`);
    $("#bf-view").textContent = content;
    $("#bf-view").classList.remove("hidden");
  } catch (e) {
    setStatus(`view failed: ${e.message}`, "err");
  }
}

async function imageInfo() {
  const image = $("#info-name").value.trim();
  if (!image) return;
  $("#info-out").textContent = "loading...";
  try {
    const data = await call("POST", "/api/image/info", { image });
    $("#info-out").textContent = JSON.stringify(data, null, 2);
  } catch (e) {
    $("#info-out").textContent = `error: ${e.message}`;
  }
}

async function imageDownload() {
  const image = $("#dl-name").value.trim();
  if (!image) return;
  $("#dl-out").textContent = "loading...";
  try {
    const data = await call("POST", "/api/image/download", { image });
    $("#dl-out").textContent = JSON.stringify(data, null, 2);
  } catch (e) {
    $("#dl-out").textContent = `error: ${e.message}`;
  }
}

async function triggerBuild() {
  const image = $("#bf-image").value.trim();
  const version = $("#bf-version").value.trim();

  // Step 1: save the current editor content to disk (so the commit below has
  // something fresh to record and the workflow's checkout sees the latest edits).
  // Step 2: commit + push the saved Dockerfile so the workflow (which checks
  // out a commit, not the working tree) sees the latest version.
  setStatus("committing + pushing…");
  let commit;
  try {
    commit = await call("POST", "/api/git/commit", {
      message: `autoimage: update Dockerfile (${image || "?"}:${effectiveVersion()})`,
    });
  } catch (e) {
    setStatus(`commit/push failed — not triggering build: ${e.message}`, "err");
    return;
  }
  if (commit.committed) {
    setStatus(`pushed ${commit.sha.slice(0, 7)} → ${commit.branch}`, "ok");
  } else {
    setStatus("no Dockerfile changes — using last committed version", "");
  }

  // Step 3: dispatch the workflow.
  setStatus("triggering build…");
  try {
    const data = await call("POST", "/api/build", { image: image || undefined, version: version || undefined });
    setStatus(`dispatched: ${data.workflow} (${data.image}:${data.version})${commit.committed ? " on " + commit.sha.slice(0, 7) : ""}`, "ok");
    if (data.image && data.version) {
      addRecentBuild(data.image, data.version);
      setAllInputs("image", data.image);
      setAllInputs("version", data.version);
    }
    // capture namespace + proxy host for the pull command helper
    if (data.namespace) _namespace = data.namespace;
    if (data.proxy_host) _proxyHost = data.proxy_host;
    updatePullCommand();
  } catch (e) {
    setStatus(`build failed: ${e.message}`, "err");
  }
}

// ---- registry download ----

const RECENT_KEY = "autoimage.recentBuilds";

// All four image/version inputs (#bf-image, #bf-version, #rd-image,
// #rd-version) are kept in sync so the user only types a value once.
const MIRROR_INPUTS = [
  { id: "bf-image",   field: "image" },
  { id: "bf-version", field: "version" },
  { id: "rd-image",   field: "image" },
  { id: "rd-version", field: "version" },
];
const _values = { image: "", version: "" };
// `version` defaults to "latest" when the user leaves the field blank.
const VERSION_DEFAULT = "latest";

function setAllInputs(field, value) {
  _values[field] = value;
  for (const m of MIRROR_INPUTS) {
    if (m.field !== field) continue;
    const el = document.getElementById(m.id);
    if (el && el.value !== value) el.value = value;
  }
  updatePullCommand();
}

function onMirrorInput(field, ev) {
  setAllInputs(field, ev.target.value);
}

/** Effective version: the user-typed value, or "latest" when blank. */
function effectiveVersion() {
  return (_values.version || "").trim() || VERSION_DEFAULT;
}

// namespace + proxy host, populated from /api/build responses. Used to
// assemble the `podman pull` helper at the bottom of the section.
let _namespace = "fantasy-mark";
let _proxyHost = "proxy.vvvv.ee";

function updatePullCommand() {
  const platform = ($("#rd-platform") || {}).value || "linux/amd64";
  const image = (_values.image || "").trim() || "<image>";
  const version = effectiveVersion();
  const cmd = `podman pull --platform ${platform} ${_proxyHost}/ghcr.io/${_namespace}/${image}:${version}`;
  const el = $("#rd-pull");
  if (el) el.textContent = cmd;
}

async function copyPullCommand() {
  const text = ($("#rd-pull") || {}).textContent || "";
  try {
    await navigator.clipboard.writeText(text);
    setRdStatus("pull command copied to clipboard", "ok");
  } catch (e) {
    // fallback: select the text
    const el = $("#rd-pull");
    if (el) {
      const r = document.createRange();
      r.selectNodeContents(el);
      const sel = window.getSelection();
      sel.removeAllRanges();
      sel.addRange(r);
      setRdStatus("press Ctrl/Cmd+C to copy", "");
    }
  }
}

function loadRecentBuilds() {
  try { return JSON.parse(localStorage.getItem(RECENT_KEY) || "[]"); }
  catch { return []; }
}

function saveRecentBuilds(list) {
  localStorage.setItem(RECENT_KEY, JSON.stringify(list.slice(0, 10)));
}

function addRecentBuild(image, version, at) {
  const list = loadRecentBuilds().filter((b) => !(b.image === image && b.version === version));
  list.unshift({ image, version, at: at || new Date().toISOString() });
  saveRecentBuilds(list);
  renderRecentBuilds();
}

function renderRecentBuilds() {
  const ul = $("#rd-list");
  ul.innerHTML = "";
  const list = loadRecentBuilds();
  if (!list.length) {
    ul.innerHTML = '<li><span class="muted">(no builds yet)</span></li>';
    return;
  }
  for (const b of list) {
    const li = document.createElement("li");
    const a = document.createElement("a");
    a.textContent = `${b.image}:${b.version}`;
    a.href = "#";
    a.onclick = (e) => {
      e.preventDefault();
      setAllInputs("image", b.image);
      setAllInputs("version", b.version);
    };
    const meta = document.createElement("span");
    const when = b.at ? new Date(b.at).toLocaleString() : "";
    meta.textContent = when;
    const dl = document.createElement("a");
    dl.textContent = "download";
    dl.href = `/api/registry/download?image=${encodeURIComponent(b.image)}&version=${encodeURIComponent(b.version)}`;
    dl.download = `${b.image}_${b.version}.tar`;
    li.appendChild(a);
    li.appendChild(meta);
    li.appendChild(dl);
    ul.appendChild(li);
  }
}

function setRdStatus(msg, kind) {
  const el = $("#rd-status");
  el.textContent = msg || "";
  el.className = "status" + (kind ? " " + kind : "");
}

async function downloadBuiltImage() {
  const image = (_values.image || "").trim();
  const version = effectiveVersion();
  if (!image) {
    setRdStatus("image is required", "err");
    return;
  }
  setRdStatus(`downloading ${image}:${version} from ghcr.io…`);
  // record the attempt; the actual response is the tarball itself
  addRecentBuild(image, version);
  const url = `/api/registry/download?image=${encodeURIComponent(image)}&version=${encodeURIComponent(version)}`;
  // Trigger a navigation-based download. The browser will follow the
  // Content-Disposition: attachment header and save the file.
  const a = document.createElement("a");
  a.href = url;
  a.download = `${image}_${version}.tar`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setRdStatus("download started — check your browser's downloads", "ok");
}

window.addEventListener("DOMContentLoaded", () => {
  $("#df-refresh").onclick = loadDockerfile;
  $("#df-save").onclick = saveDockerfile;
  $("#info-btn").onclick = imageInfo;
  $("#dl-btn").onclick = imageDownload;
  $("#bf-build").onclick = triggerBuild;
  $("#rd-btn").onclick = downloadBuiltImage;
  $("#rd-copy").onclick = copyPullCommand;
  // re-scan the editor for ${VAR} placeholders on every change
  $("#df").addEventListener("input", scanAndRenderVariables);
  $("#bv-defaults").onclick = () => {
    // fill every visible input with its default and clear if no default
    $$("#bv-list input.bv-value").forEach((inp) => {
      inp.value = inp.placeholder && inp.placeholder !== "(no default)" ? inp.placeholder : "";
    });
  };
  // re-render pull command whenever the user types or changes platform
  ["#rd-image", "#rd-version", "#rd-platform"].forEach((sel) => {
    const el = $(sel);
    if (el) el.addEventListener("input", updatePullCommand);
    if (el && sel === "#rd-platform") el.addEventListener("change", updatePullCommand);
  });
  // keep all four image/version inputs in sync — editing any one mirrors the
  // value to the other three and refreshes the pull command.
  MIRROR_INPUTS.forEach((m) => {
    const el = document.getElementById(m.id);
    if (el) el.addEventListener("input", (ev) => onMirrorInput(m.field, ev));
  });
  // seed shared state from any pre-filled values (e.g. from localStorage)
  setAllInputs("image", ($("#bf-image") || {}).value || "");
  setAllInputs("version", ($("#bf-version") || {}).value || "");
  loadDockerfile();
  loadBackups();
  renderRecentBuilds();
  updatePullCommand();
});
