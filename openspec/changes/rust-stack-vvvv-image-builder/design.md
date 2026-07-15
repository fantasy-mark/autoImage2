## Context

`AutoImage` 原本是 Python/Streamlit 单体应用，承担两件事：编辑仓库 `Dockerfile`，并通过本地 `git add/commit/push` 触发 `.github/workflows/build.yml`，由 GitHub Actions 在阿里云容器镜像仓库（`registry.cn-shenzhen.aliyuncs.com`）上构建并推送镜像。基础镜像来源固定为阿里云，由用户在 `config.json` 内填写 `repo/namespace/image/version`。

本次变更要同时解决：
1. **基础镜像来源迁移**：从阿里云切到 `proxy.vvvv.ee` 的两个 API（`/api/image/info`、`/api/image/download`），并把它们的能力直接暴露到 UI。
2. **推送目标迁移**：阿里云仓库不可用后改用 `ghcr.io`（GitHub Container Registry），登录用 `GITHUB_TOKEN`，workflow 声明 `permissions: packages: write`。
3. **前后端重写为 Rust**：用 Axum 提供 HTTP/JSON API + 静态资源，前端是单一 HTML 页面，不引入 npm 构建链。
4. **备份**：保存 `Dockerfile` 时生成带时间戳的备份文件，文件名形如 `Dockerfile.bak.YYYYMMDD-HHMMSS`，可被前端列出查看。
5. **触发器去本地化**：不再由应用进程 `git push`，而是通过 GitHub REST API（`workflow_dispatch`）远程触发 workflow，token 与元数据来自服务端配置。

约束：保留 `Dockerfile`（应用自身容器化）、保留 `.github/workflows/build.yml` 的构建/推送逻辑；不引入数据库；保持单进程部署；不引入前端构建工具。

## Goals / Non-Goals

**Goals:**
- 提供 Rust 单二进制应用，包含 HTTP/JSON API 与编辑器 UI。
- 编辑器可加载当前 `Dockerfile`、展示全部 `Dockerfile.bak.*` 历史、保存新内容（保存即生成新备份）。
- 后端代理 `proxy.vvvv.ee` 的 info / download 请求，校验输入并透传响应。
- 后端使用 GitHub PAT 调用 `POST /repos/{owner}/{repo}/actions/workflows/{workflow}/dispatches` 触发构建。
- 配置集中在 `config.toml`（GitHub token、owner、repo、workflow 文件名、proxy base URL、bind 地址、目标 registry = `ghcr.io`）。
- 保留 `.github/workflows/build.yml`，新增 `workflow_dispatch` 触发入口，并把 `repo/namespace/image/version` 从 inputs 读取；推送到 `ghcr.io`，登录凭据为 `GITHUB_TOKEN`，workflow 声明 `permissions: packages: write`。

**Non-Goals:**
- 不实现多用户、权限系统、审计日志。
- 不存储镜像（不再落盘 `.tar`），下载行为由前端按需处理。
- 不内嵌 Docker registry v2 客户端（旧的 `utils/docker_pull.py` 移除）。
- 不引入前端框架或构建工具（无 React/Vite/npm）。
- 不做 Git 仓库拉取/同步，假设运行目录即为工作副本。

## Decisions

### D1. Axum + Tokio 单进程同时承载 API 与静态资源
**选择**：单一 `cargo` crate，二进制启动时 `axum::Router` 同时挂载：
- `GET /` → `index.html` 编辑器
- `GET /static/*` → CSS/JS 静态资源
- `GET /api/dockerfile` 读取当前文件
- `GET /api/dockerfile/backups` 列出备份
- `PUT /api/dockerfile` 保存（自动备份）
- `POST /api/image/info` 代理 proxy.vvvv.ee
- `POST /api/image/download` 代理 proxy.vvvv.ee
- `POST /api/build` 触发 GitHub workflow

**理由**：避免拆分多服务；Axum 已经是 Rust 生态里轻量、稳定的 HTTP 框架。`tower-http` 提供 `ServeDir` 直接服务静态文件，省去手写路径拼接。

**备选**：
- *Actix Web*：可用但与本场景没有显著优势，依赖里 `axum + tokio` 更轻。
- *warp*：维护节奏变慢，Pass。

### D2. 前端零构建（vanilla JS + 一份 `<textarea>` 文本编辑器）
**选择**：单页 `index.html` + 一份 `app.js` + 一份 `app.css`，编辑器用 `<textarea>`，通过 CSS 提供最小化的等宽字体高亮（不引入 Monaco/CodeMirror 打包）。

**理由**：用户的核心需求是「编辑+保存+看历史」，`<textarea>` 完全够用；零构建意味着本地任何机器拉下来直接 `cargo run` 就能用。

**备选**：
- *CodeMirror 6*：用 vendored 静态资源也可以，但增大分发体积；超出当前需求。
- *Monaco*：巨大，引入 web worker；明显过度。

### D3. Dockerfile 备份文件名 `Dockerfile.bak.YYYYMMDD-HHMMSS`
**选择**：使用 `chrono::Local::now().format("%Y%m%d-%H%M%S")` 拼接。

**理由**：
- 同目录下命名，无需新建 `backups/` 子目录 → `git diff` 容易识别新文件。
- 排序即时间序（字典序 = 时间序），列文件 API 不用再排。
- 含秒级时间戳可避免高频保存碰撞。
- 本地时区（`Local`）而非 UTC，因为用户期望「我点击保存那一刻的本地时间」。

**备选**：
- *UTC + 毫秒*：跨时区协作时更稳，但和用户预期不一致。
- *Unix epoch*：不可读，UI 列表里还得二次解析。

### D4. 触发 GitHub workflow：`workflow_dispatch` + inputs
**选择**：调用
```
POST https://api.github.com/repos/{owner}/{repo}/actions/workflows/{workflow_file}/dispatches
Authorization: Bearer {GH_TOKEN}
{
  "ref": "main",
  "inputs": {
    "repo": "...",
    "namespace": "...",
    "image": "...",
    "version": "..."
  }
}
```
后端 `/api/build` 接收 `image_name`、`version` 等，从 `config.toml` 读 `repo/namespace/github.*` 并发出。

**理由**：
- 不再由应用进程 `git push`，运行机不需要 GitHub 写权限或本地 git 历史。
- 触发后用 `runs?event=workflow_dispatch` 查最新 run URL 返回给前端（可选）。
- 旧 workflow 解析 Dockerfile 第一行注释的逻辑被 inputs 取代，可读性更好。

**备选**：
- *直接 `git push`*：原方案，会把编辑器机器变成 CI 的执行人，违反「CICD 由 GitHub 自己跑」的原则。
- *`repository_dispatch`*：类型无关 workflow 配置不够直接，`workflow_dispatch` 更符合「明确触发指定 workflow」语义。

### D5. 镜像拉取：纯透传，下载产物不落盘
**选择**：`POST /api/image/download` 仅代理到 `proxy.vvvv.ee`，把 JSON 响应直接回给前端。Rust 进程不下载 tar，不在仓库内落盘。

**理由**：
- 项目原本的 `docker_pull.py` 是给 Streamlit 用户「下载到本地」用的；新流程里镜像由 GitHub Actions 在 CI 端 `docker pull` 拉取，编辑器机器没必要缓存。
- 简化后端：不必管理磁盘/清理。
- 旧 manifest 重建路径随之消失。

**备选**：
- *服务端落盘 + 暴露下载 URL*：用户没要求；增加磁盘管理负担。

### D6. 配置：`config.toml` + 环境变量覆盖
**选择**：`config.toml` 存储默认值；`APP_BIND`、`GH_TOKEN`、`PROXY_BASE_URL` 等敏感/可变值通过环境变量覆盖（`std::env::var` 优先于文件）。

**理由**：本地开发友好（文件），生产/CI 友好（env）。`GH_TOKEN` 永远走 env。

**备选**：
- *纯 env*：本地每次都得 export，体验差。
- *纯文件*：token 写进文件不安全。

### D7. 依赖固定在 `Cargo.lock`
**选择**：CI 启用 `--locked`；不指定最低 Rust 版本下限，但 `rust-toolchain.toml` 固定到 `stable`（与 GitHub `ubuntu-22.04` 默认一致）。

**理由**：避免 MSRV 漂移引入惊喜。

## Risks / Trade-offs

- **GitHub PAT 泄露** → 服务端 `config.toml` 不允许放 `GH_TOKEN`；只读 env；UI 不显示完整 token；构建用 `fine-grained` token，仅开 `Actions: write`。
- **proxy.vvvv.ee 不可用/限流** → 后端对上游 5xx 透传但记 `tracing::warn!`；不重试（避免拉长前端等待）；前端显示「上游错误」并保留表单输入。
- **`<textarea>` 体验差** → 接受换为更轻量的 CodeMirror（vendored）作为后续工作；本次不做。
- **同名秒级保存碰撞** → 1 秒内重复保存：因 1 秒分辨率理论上可能碰撞，前端禁用提交按钮至响应返回；后端若检测目标已存在则在文件名后追加 `.2`、`.3`。
- **Dockerfile 体积/特殊字符** → 备份/读取均按字节处理（`Vec<u8>`），不做编码假设；保存时强制 UTF-8 校验，失败时返回 400。
- **应用直接跑在工作副本上** → 备份与当前文件同目录，会进入 git 历史；如果用户不希望历史里出现 `Dockerfile.bak.*`，可在 `.gitignore` 中忽略 `Dockerfile.bak.*`（任务里增加此变更）。
- **触发 workflow 后失败排查** → 前端构建按钮触发后弹出 GitHub Actions 页面（构造 `https://github.com/{owner}/{repo}/actions/workflows/{workflow_file}`），用户自查；后端仅返回 `accepted: true`。

## Migration Plan

1. **添加 Rust 项目骨架**（`Cargo.toml`、`src/main.rs`、静态资源），不删旧文件。
2. **实现配置加载 + 静态服务**，可在 `cargo run` 后访问 `127.0.0.1:8080/` 看到占位页面。
3. **逐项实现 API**：先 `GET /api/dockerfile` + `PUT /api/dockerfile`（含备份）→ 再 `POST /api/image/info` → `POST /api/image/download` → `POST /api/build`。
4. **更新 `.github/workflows/build.yml`**：把第一行注释解析改为 `workflow_dispatch` inputs；保留原有 `docker build/tag/push` 步骤；新增 `permissions: { packages: write, contents: read }`，登录命令改为 `docker login ghcr.io -u ${{ github.actor }} -p ${{ secrets.GITHUB_TOKEN }}`，推送地址改为 `ghcr.io/<github-owner>/<image>:<version>`。
5. **本地联调**：用 `proofrun` / `cargo run` + 手动 `gh workflow run` 对照。
6. **切换入口文档**：`Readme.md` 改为 `cargo run --release`，并写明环境变量。
7. **删除旧文件**：`main.py`、`utils/docker_pull.py`、`github_api.py`、`config.json`、`.gitignore` 中相关行。
8. **回滚**：保留 `git log` 可回退到旧 Python 应用；新 Rust 二进制未在生产使用时无副作用。

## Open Questions

- 是否要在 `config.toml` 中支持多 registry（不再只是阿里云）？当前为简化只保留单一目标仓库。
- 是否需要把镜像 info 响应做一层 schema 归一化（仅取 `tag`/`arch` 列表）？当前决定透传。
- `POST /api/build` 是否要返回 GitHub run URL（需要再发一次 list-runs 请求）？当前决定只返回 `accepted` + 打开 Actions 页面，由用户自查。
- 是否给备份加保留策略（如仅保留最近 N 个）？当前决定保留全部，由 `.gitignore` 决定是否入库。

## ghcr.io 鉴权与开通要点

- **没有独立账号**：`ghcr.io` 完全复用 GitHub 身份，登录用户名 = GitHub 用户名，密码 = `GITHUB_TOKEN`（在 Actions 内）或 PAT（在本地/CI 外部）。
- **工作流推送**：使用 Actions 自动注入的 `GITHUB_TOKEN`，但默认权限是只读；必须在 workflow 顶部声明 `permissions: { contents: read, packages: write }`，否则 `docker push` 会 403。
- **后端触发**：需要一个 PAT 作为 `GH_TOKEN` 环境变量，作用域至少包含 `repo`（读仓库元数据）+ `workflow`（触发 workflow_dispatch）。**不要**勾选无关作用域；推荐用 fine-grained PAT，只授权目标仓库 + `Actions: Read and write`。
- **组织名空间**：若推到组织（如 `ghcr.io/my-org/...`），组织侧需 `Settings → Packages → Enable` 并在 `Settings → Actions → General → Workflow permissions` 允许工作流创建/发布包。
- **可见性**：默认私有；公开需在包页面切到 `Public`。
- **限额**：私有仓库 500MB 免费、超过按 0.25 USD/GB·月；公共仓库无存储限制。
- **第一坑**：第一次 `docker push ghcr.io/<owner>/<image>:tag` 不会自动建包——若失败 403，多半是组织未启用 Packages 或 workflow 缺 `packages: write`。
