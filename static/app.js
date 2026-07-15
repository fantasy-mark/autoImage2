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
  } catch (e) {
    if (e.status === 404) {
      $("#df").value = "";
      setStatus("no Dockerfile yet", "");
    } else {
      setStatus(`load failed: ${e.message}`, "err");
    }
  }
}

async function saveDockerfile() {
  const content = $("#df").value;
  setStatus("saving...");
  try {
    const data = await call("PUT", "/api/dockerfile", { content });
    setStatus(`saved (${data.size} bytes, backup ${data.backup})`, "ok");
    await loadBackups();
  } catch (e) {
    setStatus(`save failed: ${e.message}`, "err");
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
  setStatus("triggering build...");
  try {
    const data = await call("POST", "/api/build", { image: image || undefined, version: version || undefined });
    setStatus(`dispatched: ${data.workflow}`, "ok");
  } catch (e) {
    setStatus(`build failed: ${e.message}`, "err");
  }
}

window.addEventListener("DOMContentLoaded", () => {
  $("#df-refresh").onclick = loadDockerfile;
  $("#df-save").onclick = saveDockerfile;
  $("#info-btn").onclick = imageInfo;
  $("#dl-btn").onclick = imageDownload;
  $("#bf-build").onclick = triggerBuild;
  loadDockerfile();
  loadBackups();
});
