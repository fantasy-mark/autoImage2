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
    // remember the build so the user can download it from the registry section
    if (image && version) {
      addRecentBuild(image, version);
      // also pre-fill the download form
      $("#rd-image").value = image;
      $("#rd-version").value = version;
    }
  } catch (e) {
    setStatus(`build failed: ${e.message}`, "err");
  }
}

// ---- registry download ----

const RECENT_KEY = "autoimage.recentBuilds";

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
      $("#rd-image").value = b.image;
      $("#rd-version").value = b.version;
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
  const image = $("#rd-image").value.trim();
  const version = $("#rd-version").value.trim();
  if (!image || !version) {
    setRdStatus("image and version are required", "err");
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
  loadDockerfile();
  loadBackups();
  renderRecentBuilds();
});
