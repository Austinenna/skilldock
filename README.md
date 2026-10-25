# SkillDock

给多个 AI agent（Claude Code、Codex、WorkBuddy、OpenClaw……）统一管理 skill：在开发区写、测，确认后发布成快照，再用软链接分发给指定的 agent。

由两部分组成：

- **`skl` 命令行**（`crates/skl`）：核心逻辑都在这里，agent 也可以直接调用。
- **SkillDock 桌面应用**（Tauri）：同一套逻辑的图形界面，用来浏览、发布、调整分发和体检。

完整设计、规则和注意事项见 [docs/redesign.md](docs/redesign.md)。

## 结构

```
<root>/                 # 默认 ~/.skilldock，可用 skl init --root 指定
├── skills.toml         # [agents] 各 agent 的 skill 目录；[skills] 每个 skill 分发给谁
├── dev/<name>/         # 开发区：唯一修改源，不会被任何 agent 自动加载
├── released/<name>/    # 发布区：skl publish 写入的快照；agent 目录只链接到这里
└── _sandbox/           # skl test 用来测试自动触发
```

## 安装 skl

```bash
cargo install --git https://github.com/Austinenna/skilldock skl
skl init                      # 或 skl init --root ~/somewhere/skills
skl --help
```

常用：`skl new` / `skl test` / `skl publish <name> --to claude,codex` / `skl link <name> claude` / `skl ls --agent codex` / `skl open <name>` / `skl delete <name>` / `skl doctor` / `skl sync`。

## 桌面应用功能

- 侧栏按状态（开发中 / 有未发布改动 / 已发布）和 agent 筛选
- 详情：发布（两次点击确认）、按 agent 开关分发、取消发布、删除（移入 `.trash/`）、SKILL.md 预览、文件列表、dev / released 路径
- Doctor：断链、冲突、清单不一致、缺依赖；可预览并执行同步
- 新建 skill（写入 `dev/`）
- 三套主题、列表/网格视图、紧凑/舒适密度

已移除：AI 摘要（及 Keychain API key）、初始化迁移向导、写死的平台列表。

## 当前限制

- 文件监听只覆盖根目录、`dev/`、`released/` 顶层；skill 内部文件的修改需要手动 Refresh。
- macOS `.app` + `.dmg` 发布流程已开启；本机没有 Developer ID 证书时使用 ad-hoc 签名，正式分发前仍需 Apple notarization。
- 还没有 GitHub Actions CI。

## 首次启动

需要 macOS + Xcode Command Line Tools + Rust + Node。

```bash
# 1. 装 Rust（已装可跳过）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 2. 装前端依赖
pnpm install

# 3. 跑起来（首次冷编译 Rust ~3-5 分钟，之后秒级热重载）
pnpm tauri dev
```

应用读取 `skl` 的根目录（见上）。第一次使用前先运行 `skl init`。

## 开发命令

```bash
cd crates/skl && cargo test
pnpm build
cd src-tauri && cargo check
```

需要本地调试窗口时：

```bash
pnpm tauri dev
```

## GitHub / 提交流程

```bash
git status --short --branch
git log --oneline --decorate -5
```

当前项目以 `main` 为主线；每个 roadmap 任务完成验证后单独提交。后续再补 GitHub Actions：PR 上跑 `pnpm build`、`cargo check`、`cargo test`，tag 上跑 `pnpm release:mac`。

## 项目结构

```
skilldock/
├── crates/skl/         # 核心库 skilldock_core + skl 命令行
├── src/                # Vite + React + TS 前端
│   ├── App.tsx         # 状态与布局
│   ├── Sidebar.tsx / SkillList.tsx / Detail.tsx / Panes.tsx
│   ├── ui.tsx          # 通用组件与常量
│   ├── ipc.ts          # invoke Tauri 命令的薄封装
│   ├── palettes.ts     # 三套配色
│   └── types.ts        # 共享类型
├── src-tauri/src/
│   ├── skills.rs       # Tauri 命令：调用 skilldock_core 并转换为前端类型
│   ├── config.rs       # 界面偏好（主题、密度、视图）
│   ├── watcher.rs      # 监听根目录 / dev / released 顶层
│   └── error.rs
├── prototype/          # 早期 HTML 原型，留作设计参考
└── docs/redesign.md    # 当前设计文档
```

## 打包发布 macOS 版本

本地打包 `.app` + `.dmg`：

```bash
pnpm release:mac
```

产物会写到：

```text
src-tauri/target/release/bundle/macos/SkillDock.app
src-tauri/target/release/bundle/dmg/SkillDock_0.1.0_aarch64.dmg
src-tauri/target/release/bundle/dmg/SkillDock_0.1.0_aarch64.dmg.sha256
```

如果本机没有 Developer ID Application 证书，脚本会使用 ad-hoc 签名，适合本机验证但不会通过 Gatekeeper 公证。正式分发前先在钥匙串安装证书，并创建 notarytool profile：

```bash
SKILLDOCK_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
SKILLDOCK_NOTARY_PROFILE="skilldock-notary" \
pnpm release:mac
```

完整流程见 `DEVELOPMENT.md` §8。
