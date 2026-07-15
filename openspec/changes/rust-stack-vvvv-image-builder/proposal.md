## Why

当前 `AutoImage` 项目由 Python + Streamlit 单体应用驱动：用户在 UI 中修改 `config.json` 与 `Dockerfile`，点击构建后通过本地 `git add/commit/push` 触发 GitHub Actions，由 CICD 在阿里云容器镜像仓库（`registry.cn-shenzhen.aliyuncs.com`）上拉取基础镜像、构建并推送产物。

该流程存在三类问题：
1. **基础镜像来源被阿里云仓库绑定**，需要迁移到 `proxy.vvvv.ee` 提供的镜像查询/下载 API（`/api/image/info`、`/api/image/download`），不再依赖阿里云仓库拉取。
2. **推送目标失去了阿里云仓库**，原推送目标 `registry.cn-shenzhen.aliyuncs.com/auto_image/...` 不可用，需改用 GitHub Container Registry（`ghcr.io`），使用 `GITHUB_TOKEN` 完成登录与推送。
3. **前后端为 Python 脚本**（含子进程调用 `git push`），缺乏类型安全、跨平台分发能力，且备份/审计能力弱。本次改造使用 Rust 重写前后端，新增带时间戳的备份能力，并通过 GitHub 触发器替代直接 `git push` 的隐式提交。

本次变更同步重建运行入口与触发方式。

## What Changes

- **新增** Rust 全栈应用（单一 crate 内的二进制 + 静态资源），由 Axum 提供 HTTP 与 API、由本地 HTML/JS/CSS 提供 Dockerfile 编辑器界面。
- **新增** 镜像信息查询：调用 `https://proxy.vvvv.ee/api/image/info?image=<name>`，把响应透传给前端。
- **新增** 镜像下载：调用 `https://proxy.vvvv.ee/api/image/download?image=<name>&mode=prepare&compressed=true&platform=linux/amd64` 并把响应透传给前端。
- **新增** Dockerfile 编辑能力：列出仓库内 `Dockerfile` 与已存在的备份，前端可查看/编辑/保存；保存时先创建带时间戳的备份再写入正式文件。
- **新增** 备份命名规范：`Dockerfile.bak.YYYYMMDD-HHMMSS`（本地时区），备份目录与原文件同目录，保留全部历史。
- **新增** 触发 GitHub CICD：通过 GitHub REST API（`POST /repos/{owner}/{repo}/actions/workflows/{workflow}/dispatches`）以 `repository_dispatch` / `workflow_dispatch` 触发构建；不再由应用本地执行 `git push`。
- **新增** 配置文件：使用 `config.toml` 持久化 GitHub token、owner/repo/workflow 名、API base URL 等（替代旧 `config.json`）。
- **移除** 旧的 `main.py`（Streamlit 入口）、`utils/docker_pull.py`（Docker registry v2 直拉脚本）、`github_api.py` 占位脚本。
- **修改** `.github/workflows/build.yml`：保留镜像构建与推送逻辑，但接收来自 `workflow_dispatch` 的输入；不再依赖 Dockerfile 第一行注释解析参数；新增 `permissions: packages: write` 并改用 `GITHUB_TOKEN` 登录 `ghcr.io` 推送。
- **BREAKING** Docker 镜像拉取不再使用 `docker_pull.py` 重建 `manifest.json` 路径，下载产物改为 `proxy.vvvv.ee` 的 JSON 响应（由调用方按需处理，不在仓库内落盘 `.tar`）。
- **BREAKING** 镜像推送目标从 `registry.cn-shenzhen.aliyuncs.com/auto_image/<image>:<tag>` 改为 `ghcr.io/<github-owner>/<image>:<tag>`，登录凭据由 `DOCKER_USERNAME`/`DOCKER_PASSWORD` 切换为 `GITHUB_TOKEN`。

## Capabilities

### New Capabilities

- `image-info-lookup`: 通过 `proxy.vvvv.ee/api/image/info` 查询镜像元数据，前端可输入镜像名获得版本列表/架构清单。
- `image-download`: 通过 `proxy.vvvv.ee/api/image/download` 触发镜像下载请求并把响应透传给前端。
- `dockerfile-editor`: 提供在线查看/编辑/保存仓库 `Dockerfile` 的能力，每次保存先写带时间戳的备份再覆盖正式文件。
- `dockerfile-backup`: 定义备份文件命名规则、目录位置、保留策略与历史查看接口。
- `cicd-trigger`: 提供后端 API 触发 GitHub Actions workflow 重新构建与推送镜像；携带本次保存的元数据（提交信息、镜像名、版本）。
- `rust-app-config`: 应用配置加载（GitHub token、repo、workflow 名、API base URL 等），支持从 `config.toml` 加载并提供只读运行时视图。

### Modified Capabilities

（无 — 当前 `openspec/specs/` 为空，本次为首次定义能力集；旧 Python 实现不作为正式能力保留。）

## Impact

- **代码**：新增 `src/` Rust crate，删除 `main.py`、`utils/docker_pull.py`、`github_api.py`、`config.json`；保留并修改 `Dockerfile`（应用容器化）、`.github/workflows/build.yml`。
- **依赖**：新增 Rust 工具链（`rustc`、`cargo`），Cargo 依赖含 `axum`、`tokio`、`reqwest`、`serde`、`toml`、`tracing`、`chrono`、`anyhow`、`thiserror`；前端零构建依赖（vanilla JS + 一份简单的代码编辑器，如 `<textarea>` + 高亮由 CSS 完成）。
- **CI/CD**：GitHub Actions 仍负责镜像构建与推送，但触发方式从 `push`（被动）改为 `workflow_dispatch`（主动）；构建所需的 `repo/namespace/image/version` 从 workflow inputs 读取；推送到 `ghcr.io` 由 `GITHUB_TOKEN` 鉴权，workflow 必须声明 `permissions: packages: write`。
- **配置/凭据**：触发后端需要 `GH_TOKEN`（PAT，`repo` + `workflow` 范围）作为环境变量；不再使用 `DOCKER_USERNAME`/`DOCKER_PASSWORD`，避免把第三方镜像仓库凭据继续留在 secrets 里。
- **运行时**：Rust 二进制默认监听 `127.0.0.1:8080`（可通过 `APP_BIND` 覆盖），提供 UI 与 JSON API；仍以容器或本地进程方式运行。
