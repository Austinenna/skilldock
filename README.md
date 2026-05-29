# SkillLoom

集中式 Skill 管理器 —— 所有 skill 真实存放在 `~/.skillloom/skills/`，各 AI 工具的目录里只放符号链接。

**当前 MVP 功能：**

- 三栏 macOS 原生窗口（侧栏 / 列表 / 详情）
- 扫描 `~/.skillloom/skills/`，列出所有 skill
- 解析 `SKILL.md` frontmatter：`name` / `description` / `version` / `tags`
- 详情接口返回 `SKILL.md`，详情页显示真实 source path 和文件列表
- 按平台开关路由（创建/删除 symlink）
- Import 新 skill（写入 SKILL.md 模板）
- Delete skill（清理所有 symlink + 真目录）
- 文件监听：central / 可见平台目录变化后自动刷新，保留手动 Refresh 兜底
- 路径和 skill id 安全校验，避免路径穿越和误删真实目录
- 非阻塞错误通知和操作 pending 状态
- 三套主题、列表/网格视图、紧凑/舒适密度（持久化到 `~/Library/Application Support/com.skillloom.desktop/config.json`）
- API key 可存到 macOS Keychain，前端只读取 configured/not-configured 状态
- AI 摘要支持 Anthropic Messages 和 Chat Completions 两种自定义端点；按 `SKILL.md` 内容 hash + provider/model 缓存
- 没有配置 key 时，AI 摘要会降级显示 frontmatter description

下一阶段再做：签名/公证后的正式分发、CI release workflow、批量路由。

## 当前限制

- AI 摘要的 live API 请求需要用户在 Settings 里配置 provider、endpoint、model 和 API key；API key 不写入配置文件。
- macOS `.app` bundle 已开启，但当前是 unsigned local build；正式发给别人前还需要 Developer ID 签名、notarization 和 DMG/release 流程。
- watcher 只在启动时读取一次当前隐藏平台配置；运行中修改平台可见性后，仍可用手动 Refresh 兜底。
- 还没有 GitHub Actions CI / release workflow。

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

第一次启动会自动创建空目录 `~/.skillloom/skills/`。点 ＋ Import 加 skill 试试，或者把你已有的 skill 复制进去。

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

当前项目以 `main` 为主线；每个 roadmap 任务完成验证后单独提交。后续发布前再补 GitHub Actions：PR 上跑 `pnpm build`、`cargo check`、`cargo test`，tag 上跑 Tauri 打包。

## 项目结构

```
SkillLoom/
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
│       ├── ai.rs       # Keychain API key + AI summary cache
│       ├── watcher.rs  # 文件监听，emit skills-changed
│       └── error.rs    # 统一错误类型
├── docs/plans/         # 分步实现计划
├── docs/reports/       # 每步修改报告
└── DEVELOPMENT.md      # 完整开发文档
```

## 打包成 .app

本地 unsigned build：

```bash
pnpm tauri build
```

`.app` 产物会写到 `src-tauri/target/release/bundle/macos/SkillLoom.app`。这个构建适合自己机器验证；正式分发给别人前，需要配置 Apple Developer ID 签名、notarization 和 DMG/release 流程。完整流程见 `DEVELOPMENT.md` §8。
