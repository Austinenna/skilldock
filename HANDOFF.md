# SkillDock 交接记录

## 当前定位

SkillDock 是一个 macOS 桌面端 Skill 管理器。它把真实 Skill 集中放在 `~/.skilldock/skills/`，再通过符号链接按平台路由到 Claude Code、Codex CLI、OpenClaw 等工具目录。前端使用 Vite + React + TypeScript，桌面壳和文件系统操作使用 Tauri 2 + Rust。

正式项目路径：`~/Projects/apps/skilldock`

## 已完成

- 项目从 `~/ClaudeCodeProjects/SkillLoom` 迁入 `~/Projects/apps/skilldock`，保留原 Git 历史和 `main` 分支。
- 产品名、前端包名、Rust crate、Tauri product name、Bundle ID、发布脚本、测试临时路径和文档统一改为 SkillDock。
- 中央 Skill 目录从 `~/.skillloom` 改为 `~/.skilldock`，迁移后核对了 2,864 个目录/文件/链接条目和约 76.8MB 文件内容。
- 已更新 `.cc-switch/skills/webserp` 和 `.cc-switch/skills/cli-anything` 两个指向中央目录的符号链接。
- 应用偏好从 `~/Library/Application Support/com.skillloom.desktop/` 迁移到 `~/Library/Application Support/com.skilldock.desktop/`。
- 保留并纳入当前工作树中已有的 macOS `.app` + `.dmg` 发布脚本和 v0.1.0 发布记录。

## 当前实现

- 扫描中央 Skill 目录并解析 `SKILL.md` frontmatter。
- 查看 Skill 详情、文件清单和真实 source path。
- Import / Delete Skill。
- 按平台新增或删除符号链接路由。
- 初始化中央仓库时预览来源、冲突和迁移动作。
- 文件监听后自动刷新，手动 Refresh 作为兜底。
- AI 摘要支持 Anthropic Messages 和 Chat Completions 自定义端点，并按内容 hash、provider、model、endpoint 缓存。

## 已知限制

- live AI 摘要仍需在 Settings 中临时配置 endpoint、model 和 API key。
- 本机没有 Developer ID 证书时，发布包为 ad-hoc 签名，不能直接通过 Gatekeeper 公证分发。
- 尚未建立 GitHub Actions CI；当前验证在本机执行。
- watcher 启动时读取一次平台可见性；运行中修改可见性后可手动 Refresh。

## 接下来

1. 运行 `pnpm build`、`cd src-tauri && cargo check`、`cd src-tauri && cargo test`。
2. 检查 `git diff` 和待提交文件，确认没有凭据、构建缓存或不应入库的本地数据。
3. 提交 SkillDock 正式开发基线。
4. 创建新的远程 `skilldock` 仓库并推送；旧的 SkillLoom 远程不再作为项目入口。
