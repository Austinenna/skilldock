# SkillLoom

集中式 Skill 管理器 —— 所有 skill 真实存放在 `~/.skillloom/skills/`，各 AI 工具的目录里只放符号链接。

**v1 MVP 功能：**

- 三栏 macOS 原生窗口（侧栏 / 列表 / 详情）
- 扫描 `~/.skillloom/skills/`，列出所有 skill
- 按平台开关路由（创建/删除 symlink）
- Import 新 skill（写入 SKILL.md 模板）
- Delete skill（清理所有 symlink + 真目录）
- 三套主题、列表/网格视图、紧凑/舒适密度（持久化到 `~/Library/Application Support/com.skillloom.desktop/config.json`）

v2 再做：AI 摘要、文件监听、批量路由。

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
│       └── error.rs    # 统一错误类型
└── DEVELOPMENT.md      # 完整路线图（Phase 0–9）
```

## 打包成 .app

v1 暂未开启打包（`tauri.conf.json` 里 `bundle.active: false`）。
要分发时把它改成 `true` 并补 `bundle.icon`，然后 `pnpm tauri build`。完整签名 + notarization 流程见 `DEVELOPMENT.md` §8。
