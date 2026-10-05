# SkillDock 项目约定

## 接手顺序

开始新的开发或排查前，先阅读 `README.md` 和 `HANDOFF.md`，再根据任务需要阅读 `DEVELOPMENT.md` 与对应源码。`docs/plans/` 和 `docs/reports/` 是方案与过程记录，不替代当前实现。

## 项目边界

SkillDock 是 macOS 桌面端 Skill 管理器。中央真实目录是 `~/.skilldock/skills/`；Claude Code、Codex CLI、OpenClaw 等平台目录只保存指向中央目录的符号链接。涉及导入、删除、路由或初始化时，必须先确认链接目标和路径 containment，不能把真实平台目录当作可覆盖内容。

## 验证

常规验证使用：

```bash
pnpm build
cd src-tauri && cargo check
cd src-tauri && cargo test
```

需要发布包时使用 `pnpm release:mac`；没有 Developer ID 证书时只用于本机 ad-hoc 验证。测试不得写入真实中央目录，优先使用 Rust 测试中的临时目录或独立 fixture。

## 路径与凭据

正式项目路径为 `~/Projects/apps/skilldock`。应用偏好位于 `~/Library/Application Support/com.skilldock.desktop/`。API key 只在当前 App 会话中使用，不写入仓库、配置文件或日志；不要把真实密钥放入测试、示例或提交历史。
