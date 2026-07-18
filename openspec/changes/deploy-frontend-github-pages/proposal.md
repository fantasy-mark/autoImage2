## Why

The Rust/Axum 后端 (`autoimage.exe`) 跑在用户的 Windows 主机上，所有持久化（Dockerfile、备份、git 历史）都依赖本机文件系统和服务进程。要分享给同事或者换台电脑使用就得重新搭一遍。本次改动的目标是把前端 + 业务逻辑拆成**纯前端**，能静态托管在 GitHub Pages 上，所有调用直接打 `api.github.com` 和 `ghcr.io`。

## What Changes

- 把 `static/` 目录搬到 `gh-pages` 分支（或同级 `index.html`），去掉对后端的依赖
- 删除 `/api/dockerfile`/`/api/image/*` 等读写本地文件的端点
- 用浏览器直接调 GitHub Contents API（`PUT /repos/{o}/{r}/contents/Dockerfile`）读写 Dockerfile，自动 commit
- 用浏览器直接调 `POST /repos/{o}/{r}/actions/workflows/build.yml/dispatches` 触发 workflow
- "backups" 用 GitHub commits API 渲染历史版本（替代本地 `.bak.*` 文件）
- 删掉 `/api/image/info` 和 `/api/image/download`（这两个服务 `proxy.vvvv.ee`，原计划是给后端用）
- 在前端**仅放一个 `proxy.vvvv.ee` 文字链接**作为参考，pull 命令可能也用到
- 删除 `/api/registry/download` 的**浏览器版**实现（需要拼 docker save tar，工作量大），`ghcr-pull-tar` 只留 pull 命令生成 + 显示，下载让用户跑 `podman pull`
- 用户在 UI 输入 GH_TOKEN，存 `localStorage`（PAT 暴露在 devtools 是已知 trade-off）
- 后端 Rust 代码保留在 `main` 分支（不动），让用户自选本地开发还是 Pages 部署

## Capabilities

### New Capabilities

- `github-pat-auth`：浏览器存/取 GH_TOKEN，所有 fetch 自动加 `Authorization: Bearer …` 头；token 失效时引导用户重输
- `dockerfile-editor`：textarea 编辑器、显示当前内容、提示 unsaved changes
- `dockerfile-contents-api`：通过 GitHub Contents API 读写 `Dockerfile`（包括触发自动 commit）；按需显示 latest commit 的 SHA
- `commit-history-diff`：调 commits API 列出最近 N 次 `Dockerfile` 的变更，可选 viewing the diff between two revisions
- `workflow-build-trigger`：浏览器直接 `POST .../actions/workflows/build.yml/dispatches` 触发 build；镜像/版本输入镜像到所有相关输入框
- `gh-pages-static-host`：把 `index.html` + `app.js` + `app.css` 部署到 `<owner>.github.io/autoimage2` 分支，开启 Pages
- `ghcr-pull-tar`：根据输入的 image/version/platform 拼接 `podman pull` 命令（含可选的 `proxy.vvvv.ee/ghcr.io` 路径），提供 Copy 按钮

### Modified Capabilities

（None：项目第一次走这套 spec-driven 流程；现有 main 里的功能被废弃而不是改造；如未来需要回退，从 git history 取就行）

## Impact

- **新增**：`gh-pages` 分支 + 浏览器版 `app.js`
- **删除**（按"页面里不再出现这些代码"衡量）：
  - `src/handlers/dockerfile.rs`、`src/handlers/backup.rs`
  - `src/handlers/proxy.rs` 中 image info/download 处理
  - 前端 `/api/dockerfile` 那块 UI（替换为 Contents API 调用）
- **保留**：`main` 分支的 Rust 后端完整不动；`static/` 里的 CSS 可以复用
- **依赖**：
  - `api.github.com`（用户输入的 PAT）
  - `ghcr.io`（public 拉镜像，无需 auth）
  - `proxy.vvvv.ee` 仅作为页面上一个文字链接
- **安全 trade-off**：GH_TOKEN 存在浏览器 `localStorage`，DevTools 可见；不适合分享给不信任的人
