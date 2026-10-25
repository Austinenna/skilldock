# SkillDock 项目约定

## 接手顺序

开始新的开发或排查前，先阅读 `README.md` 和 `HANDOFF.md`，再根据任务需要阅读 `DEVELOPMENT.md` 与对应源码。`docs/plans/` 和 `docs/reports/` 是方案与过程记录，不替代当前实现。

## 项目边界

**正在重构**，以 `docs/redesign.md` 为准。新结构：Skill 仓库根目录由 `SKILLDOCK_HOME` > `~/.config/skilldock/config.toml` 的 `root` > `~/.skilldock` 决定（作者本机为 `~/Projects/skills`），其中 `dev/` 开发、`released/` 发布、`skills.toml` 为分发清单；各 agent 目录只保存指向 `released/` 的软链接。核心逻辑与 `skl` 命令行在 `crates/skl`。

GUI（`src-tauri/` + `src/`）只是 `crates/skl` 的外壳：新的 skill 逻辑一律加在 `crates/skl`，Tauri 层只做类型转换。涉及链接的操作只允许创建、修正、删除指向 `<root>/released/` 的软链接，agent 目录里的真实目录和外部链接只报告不修改。

## 验证

常规验证使用：

```bash
cd crates/skl && cargo test
pnpm build
cd src-tauri && cargo check
cd src-tauri && cargo test
```

需要发布包时使用 `pnpm release:mac`；没有 Developer ID 证书时只用于本机 ad-hoc 验证。测试不得写入真实的 skl 根目录或 agent 目录：用 `SKILLDOCK_HOME=<临时目录>`，并在临时 `skills.toml` 里把 `[agents]` 指向临时路径。

## 路径与凭据

正式项目路径为 `~/Projects/apps/skilldock`。应用偏好位于 `~/Library/Application Support/com.skilldock.desktop/`。不要把任何真实凭据放入测试、示例或提交历史。
