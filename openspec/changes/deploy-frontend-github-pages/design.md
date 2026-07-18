## Context

今天 2026-07-16，项目当前形态是：

- 后端 Rust/Axum 单二进制 `autoimage.exe`，跑在 Windows 主机上
- 前端纯静态（HTML/CSS/JS），已挂在 `static/`，由后端 ServeDir 直接吐
- 持久化完全依赖本机文件系统：`Dockerfile`、`.bak.<timestamp>` 备份、本地 git 仓
- 网络侧有 `digest_proxy` 桥解决 reqwest 不支持 Digest 代理的问题

用户目标：把前端 + 业务逻辑拆成纯前端，部署到 GitHub Pages；后端 Rust 二进制保留在 `main` 分支作为可选的本地开发形态。

## Goals / Non-Goals

**Goals:**

- 一个浏览器 → 一个 GH_TOKEN → 直接跟 GitHub / ghcr.io 对话
- Dockerfile 编辑、保存、commit 全走 GitHub Contents API（PUT 自带 commit，省掉本地 git 操作）
- Workflow dispatch 也直调 GitHub API
- "Backups" 由 GitHub commits API 提供历史视图，替代本地 `.bak.*` 文件
- pull 命令显示 + 复制，docker save tar 解析不再做（用户跑 `podman pull` 解决）
- `proxy.vvvv.ee` 留一个文本链接作为参考
- 前端相对路径，能跑在 `<owner>.github.io/autoimage2/` 下

**Non-Goals:**

- 不做后端兼容 shim（Rust 后端保留 main 分支，但不写适配层）
- 不支持 private 仓库的 PR 检查（public 仓库已够用）
- 不实现 docker save tar 流（用户跑 `podman pull`）
- 不做 OAuth 流程（PAT 直接输入浏览器，trade-off 接受）
- 不引入 framework（继续原生 JS，避免 vendor lock）

## Decisions

### 1. 不引入 npm / framework

**Decision**：保持 `static/app.js` 为零依赖原生 JS。

**Rationale**：用户场景只是单页工具，原生 JS 完全够用；少一层供应链 = 少一个攻击面 + 少一次构建。`diff` 算法手写 ~30 行（最长公共子序列）即可。

**Alternatives considered**：
- React/Vue：成本不划算
- 引入 `diff` npm 包：要 bundle，部署复杂度上升
- 引入 `@octokit/rest`：Octokit 包 ~250KB，浏览器加载慢，且大部分 API 用不上

### 2. PAT 存 `localStorage` 而不是 `sessionStorage` / cookie

**Decision**：用 `localStorage`，明确告知用户 token 是"无加密，DevTools 可见"。

**Rationale**：用户每天触发多次 build，sessionStorage 每次重输太烦；cookie 要 HTTPS-only，纯静态 Pages 也设不了 HttpOnly。

**Alternatives considered**：
- `sessionStorage`：频繁重输不可接受
- 弹出 modal 每次输入：同上
- 加密存 IndexedDB：密钥在哪？浏览器本来就暴露了

### 3. Contents API PUT 用 `sha` 字段做并发检测

**Decision**：每次 save 都带上"上次 fetch 的 SHA"做 optimistic locking；服务器冲突时返回 409，UI 提示刷新。

**Rationale**：用户可能在多个浏览器 tab 同时编辑；不加 sha 的话后写覆盖前写。

**Alternatives considered**：
- 永远强制覆盖：会丢用户的改动
- Lock file：纯前端不好实现

### 4. Drop 后端的 image info / image download / registry download

**Decision**：这三个端点不进前端版。

**Rationale**：
- image info/download：原本转发 `proxy.vvvv.ee`，而 `proxy.vvvv.ee` 大概率没 CORS，纯前端调不通。删掉省事
- registry download：要在浏览器拼 docker save tar（manifest + 多 layer 流式下载 + tar 打包），300+ 行 JS 起步，但用户说"不需要"，那就省了
- 留一个 `proxy.vvvv.ee` 文本链接 + podman pull 命令模板，把"拉镜像"的活儿交还给用户本地命令行

**Alternatives considered**：
- 写一个 Cloudflare Worker 桥接 proxy.vvvv.ee：增加成本和依赖，用户没要求
- 拼 tar：在浏览器里有 quota 风险（GB 级别镜像会爆），延迟体验差

### 5. `gh-pages` 分支 = 只含静态文件

**Decision**：`gh-pages` 分支里只有 `index.html` + `app.css` + `app.js` + `.nojekyll`。

**Rationale**：避免 Cargo target、git 子模块之类的杂物进 Pages。`.nojekyll` 跳过 Jekyll 转换（不然下划线开头的文件会被吞）。

### 6. 编辑器 vs 内容寻址：用 Contents API 的 `ref=sha` 而不复制内容寻址

**Decision**：保持 GitHub Contents API 模型（commit 哈希 = SHA-1 of git tree），不引入 IPFS/CAS。

**Rationale**：用户已经在用 Git + GitHub，没必要换。

## Risks / Trade-offs

- **PAT 暴露** → 接受：UI 顶部永久 banner 提醒"token 在 DevTools 可见，请勿分享给不信任的人"。如果以后想给团队用，再迁移到方案 B（Cloudflare Worker 持 PAT）。
- **proxy.vvvv.ee 不能直连** → 接受：drop 转发端点，pull 命令走用户自己的网络路径。
- **`diff` 库手写** → 风险：大文件 diff 性能差（O(n²)）。接受：Dockerfile 通常 < 1k 行，Myers 算法也只用 ~30 行代码。
- **PAT 401 时整个 UI 瘫痪** → 接受：401 后立刻弹 token 输入框，清除 localStorage。
- **GitHub Contents API 限流** → 风险：5000 次/小时（带 token）。用户每天 build 几十次不会触及。

## Migration Plan

1. **写完前端**：在 `main` 分支建 `ghpages/index.html` + `ghpages/app.js` + `ghpages/app.css`，把后端 API 调用改成 GitHub Contents API / Actions API / ghcr.io 调用
2. **本地验证**：用 `python -m http.server` 起在 `ghpages/` 目录，浏览器打开 `http://localhost:8000`，跑通完整流程
3. **建 gh-pages 分支并推**：`git checkout --orphan gh-pages; git checkout ghpages/ .; git commit; git push -f`
4. **开启 Pages**：仓库 Settings → Pages → source = `gh-pages` branch / root
5. **验证**：浏览器访问 `https://<owner>.github.io/autoimage2/`，输入 PAT，编辑 Dockerfile，触发 build，确认 run 起来
6. **回滚**（如果出大问题）：把 Pages source 切回 `main` 即可，`main` 里的后端仍可用

## Open Questions

- 用户要不要 OAuth 流程换掉 PAT？目前假设**不要**（最快上线）。
- `gh-pages` 分支是 `--orphan` 独立（不和 main 共享 history）还是基于 main 切出来？打算用 `--orphan`。
- Dockerfile 编辑历史是直接调 GitHub commits API（用户每次点开 history 才拉），还是预加载到内存？打算按需拉，单次拉 20 条。
