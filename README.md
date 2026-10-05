# SkillDock

集中式 Skill 管理器 —— 所有 skill 真实存放在 `~/.skilldock/skills/`，各 AI 工具的目录里只放符号链接。

**当前 MVP 功能：**

- 三栏 macOS 原生窗口（侧栏 / 列表 / 详情）
- 扫描 `~/.skilldock/skills/`，列出所有 skill
- 解析 `SKILL.md` frontmatter：`name` / `description` / `version` / `tags`
- 详情接口返回 `SKILL.md`，详情页显示真实 source path 和文件列表
- 按平台开关路由（创建/删除 symlink）
- 初始化中央仓库：扫描所有已知平台 skill 目录，预览来源/冲突后把真实目录备份迁移到 central 并替换为 symlink
- Import 新 skill（写入 SKILL.md 模板）
- Delete skill（清理所有 symlink + 真目录）
- 文件监听：central / 可见平台目录变化后自动刷新，保留手动 Refresh 兜底
- 路径和 skill id 安全校验，避免路径穿越和误删真实目录
- 非阻塞错误通知和操作 pending 状态
- 三套主题、列表/网格视图、紧凑/舒适密度（持久化到 `~/Library/Application Support/com.skilldock.desktop/config.json`）
- API key 只在当前 App 会话内临时使用，不写入配置文件，也不保存到 macOS Keychain
- AI 摘要支持 Anthropic Messages 和 Chat Completions 两种自定义端点；Settings 支持连接测试
- AI 摘要按 `SKILL.md` 内容 hash + provider/model/endpoint 缓存
- 没有配置 key 时，AI 摘要会降级显示 frontmatter description

下一阶段再做：Developer ID 证书接入后的正式公证、CI release workflow、批量路由。

## 当前限制

- AI 摘要的 live API 请求需要用户在 Settings 里配置 provider、endpoint、model，并粘贴本次会话使用的 API key；API key 不持久化，可用 Test 按钮先验证。
- macOS `.app` + `.dmg` release 流程已开启；本机没有 Developer ID 证书时会使用 ad-hoc 签名，正式分发前仍需要 Apple notarization。
- watcher 只在启动时读取一次当前隐藏平台配置；运行中修改平台可见性后，仍可用手动 Refresh 兜底。
- 还没有 GitHub Actions CI；当前 release 流程先在本机执行。

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

第一次启动会自动创建空目录 `~/.skilldock/skills/`。点 ＋ Import 加 skill 试试，或者把你已有的 skill 复制进去。

## 开发命令

```bash
pnpm build
cd src-tauri && cargo check
cd src-tauri && cargo test
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
├── prototype/          # 之前的 HTML/React CDN 原型，留作设计参考
├── src/                # Vite + React + TS 前端
│   ├── App.tsx         # 所有 UI 组件
│   ├── ipc.ts          # invoke Tauri 命令的薄封装
│   ├── palettes.ts     # 三套配色
│   └── types.ts        # 共享类型
├── src-tauri/          # Rust 后端
│   └── src/
│       ├── platforms.rs # 平台清单（central + claude + codex + ...）
│       ├── skills.rs   # scan / import / delete
│       ├── routes.rs   # add_route / remove_route（symlink 管理）
│       ├── config.rs   # 用户偏好持久化
│       ├── ai.rs       # 会话内 API key 请求 + AI summary cache
│       ├── watcher.rs  # 文件监听，emit skills-changed
│       └── error.rs    # 统一错误类型
├── docs/plans/         # 分步实现计划
├── docs/reports/       # 每步修改报告
└── DEVELOPMENT.md      # 完整开发文档
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
