# SkillDock 重构方案：开发 / 发布分离 + `skl` 命令行

> 2026-10-25 定稿。这份文档是这套 skill 管理流程的唯一说明，换电脑、换 agent、过几个月忘了，都以这里为准。

## 1. 要解决的问题

1. 本机 skill 分散在 `~/.skilldock/skills`、`~/Projects/skills`、`~/.claude/skills`、`~/.codex/skills`、`~/.workbuddy/skills`、`~/.agents/skills` 等处，同一个 skill 有多份副本且内容不一致。
2. 想让一个 skill 被多个 agent 使用（Claude Code、Codex、WorkBuddy、OpenClaw，以后加 Kairos）。
3. 新 skill 需要先测试、再“转正”，原 SkillDock 没有这个流程——它只有一个中央目录，进去就是正式版。
4. 原 SkillDock 偏重：GUI 必须打开才能用，agent 无法调用；AI 摘要不好用；初始化迁移代码占了一半。

## 2. 核心原则（不可违反）

1. **开发区和发布区分开。** `dev/<name>` 随便改；`released/<name>` 只由 `skl publish` / `skl adopt` 写入，**不手动编辑**。
2. **发布 = 复制一份快照**，不是链接到开发目录。开发中的修改不会影响正在使用的版本。
3. **agent 目录里只放软链接，且只指向 `released/`**，永远不指向 `dev/`。
4. **清单 `skills.toml` 是“谁用哪些 skill”的唯一依据。** 软链接是由清单生成的结果，丢了就 `skl sync` 重建，不需要备份链接本身。
5. **发布需要用户明确同意。** 测试通过 ≠ 获得发布授权；agent 不得自行 `skl publish`（与 `~/.codex/AGENTS.md` 的约定一致）。
6. **`skl` 只动它自己创建的东西。** agent 目录里的真实目录、指向别处的软链接、各 agent 自带的系统目录，一律只报告、不修改、不删除。

## 3. 目录结构

```
<root>/                       # 一个 git 仓库（私有）
├── skills.toml               # 清单：agent 列表 + 每个 skill 分发给谁
├── dev/<name>/               # 开发区：唯一修改源；任何 agent 都不会自动加载
├── released/<name>/          # 发布区：稳定快照；agent 的软链接只指向这里
├── _sandbox/                 # skl test 生成的测试项目（.gitignore）
├── .gitignore
└── README.md
```

### 根目录在哪

`skl` 按以下优先级确定 `<root>`：

1. 环境变量 `SKILLDOCK_HOME`
2. `~/.config/skilldock/config.toml` 中的 `root = "..."`（`skl init --root <path>` 会写入）
3. 默认 `~/.skilldock`

- **软件默认值**是隐藏目录 `~/.skilldock`：给别人用时，大多数人只是装 skill，不开发，放隐藏目录不打扰。
- **作者本机**：`root = "~/Projects/skills"`，开发区在日常工作目录里，方便编辑和让 agent 按路径读取。
- `~/.skilldock/` 在作者本机不再存放 skill，只保留 `archive/`（迁移备份）和 GUI 的界面配置。

## 4. 流程

```
skl new <name>          在 dev/ 建骨架
   │  编辑 dev/<name>/
   ▼
测试一：功能          把 dev/<name>/SKILL.md 的完整路径给 AI，让它按路径读取后执行代表性任务。
                      验证的是指令和脚本，不验证自动触发。
   ▼
测试二：自动触发      skl test <name>  → 在 _sandbox/ 的项目级目录里链接开发版，
                      在 _sandbox 里启动 agent，看 description 能否被正确触发。
                      skl test <name> --clean 清理。
   ▼
用户确认可以发布
   ▼
skl publish <name> --to claude,codex      复制快照到 released/，写清单，同步链接，git 提交
   ▼
之后修改：继续改 dev/ → 测试 → 用户确认 → 再次 skl publish <name>（不带 --to 则沿用清单里的 agent）
```

**测试隔离靠的是“发布才复制”**，不是靠目录位置。`dev/` 和 `released/` 在同一个仓库里只是为了一次 clone 全带走。

## 5. `skills.toml`

```toml
[agents]
claude    = "~/.claude/skills"
codex     = "~/.codex/skills"
workbuddy = "~/.workbuddy/skills"
openclaw  = "~/.openclaw/skills"
# kairos  = "~/.kairos/skills"      # Kairos 接入 skill 后取消注释

[publish]
# 发布时不复制的文件/目录名（按名称匹配，任意层级）
exclude = [".DS_Store", "__pycache__", ".git"]

[skills]
wechat-gzh   = ["claude"]
draft-to-app = ["codex"]
```

**发布 = 原样复制，只排除垃圾文件。**原则是“测的是什么，发的就是什么”：`node_modules/`、`tests/` 等都照样复制。`node_modules` 不进 git（`.gitignore` 排除），但这只影响换电脑（见第 8 节），不影响本机发布。

> 2026-10-25 修订：最初默认还排除 `tests` 和 `node_modules`。这样做会让依赖 npm 的 skill（如 `wechat-styles`）发布后缺依赖、无法运行，为此还设计过 `setup.sh` + 发布拦截。后来认定排除依赖本身就是错误决定，改为只排除垃圾文件，`setup.sh` 方案不再需要。

**Python 例外**：`.venv` 内写死了创建时的绝对路径，复制到 `released/` 后会失效。Python skill **不要在 skill 目录里建 `.venv`**，改用 `uv run`，并在脚本开头用 PEP 723 内联写明依赖（`# /// script` 块），运行时自动安装。

另有可选的 `[doctor] ignore = ["claude/synced", ...]`：agent 自己维护的条目（格式 `agent/条目名`），`doctor` 不再报告。

- `[skills]` 里列出的是**已发布并分发**的 skill。只在 `dev/` 里、未列出的 skill 不会出现在任何 agent。
- 可以手改清单再 `skl sync`；`skl publish --to` / `skl unpublish` 会自动改清单（保留注释和格式）。
- 某个 skill 写成 `[]` 表示已发布但暂不分发给任何 agent。

## 6. `skl` 命令

| 命令 | 作用 |
|---|---|
| `skl init [--root PATH]` | 创建根目录结构；带 `--root` 时把路径写入 `~/.config/skilldock/config.toml`。已存在的文件不覆盖 |
| `skl root` | 打印当前根目录及其来源 |
| `skl new <name>` | 在 `dev/` 创建 `SKILL.md` 骨架 |
| `skl ls` | 列出所有 skill：状态（开发中 / 已发布 / 开发版有未发布改动）+ 分发到哪些 agent |
| `skl test <name> [--clean]` | 在 `_sandbox/.claude/skills`、`_sandbox/.agents/skills` 下链接**开发版**，用于测试自动触发 |
| `skl publish <name> [--to a,b]` | dev → released 快照；更新清单；同步该 skill 的链接；根目录是 git 仓库时自动提交 |
| `skl adopt <path> [--as name] [--to a,b]` | 把一个已有目录原样收进 `released/`（迁移旧副本、收录第三方 skill 用）；`dev/` 里没有同名时也复制一份到 `dev/` |
| `skl unpublish <name>` | 从清单移除、删除链接和 `released/<name>`；`dev/` 保留 |
| `skl sync [--dry-run]` | 让所有 agent 目录的链接与清单一致 |
| `skl doctor` | 体检：断链、冲突、清单与发布区不一致、命名问题、开发版未发布改动 |

### `sync` 的安全规则

对每个 agent 目录：

| 目录里现有的东西 | 清单要求有 | 清单不要求 |
|---|---|---|
| 不存在 | 创建软链接 | — |
| 指向 `<root>/released/<name>` 的软链接 | 保持 | **删除**（这是 skl 管理的） |
| 指向 `<root>/released/` 下其他位置、或已失效的旧 skl 链接 | 修正 | 删除 |
| 真实目录 / 文件 | **冲突，跳过并报告** | 不动 |
| 指向别处的软链接 | **冲突，跳过并报告** | 不动 |

- agent 目录不存在、但其上级目录存在（说明 agent 已安装）时，创建 `skills/` 目录；上级目录也不存在时跳过并提示。
- 以 `.` 开头的条目（`.system`、`.DS_Store`、各 agent 的迁移标记文件）永远不碰。
- `~/.claude/skills/synced` 等 agent 自己维护的目录不是软链接，按规则天然不会被动到。

### `publish` 的原子性

先复制到 `released/.staging-<name>`，再把旧版本改名为 `released/.old-<name>`，把新版本改名到位，最后删除旧版本。软链接指向的路径 `released/<name>` 不变，agent 不会看到半成品目录。

### 命名规则

skill 名（目录名）必须是小写字母、数字、连字符：`^[a-z0-9][a-z0-9-]*$`，且与 `SKILL.md` frontmatter 的 `name` 一致。`doctor` 会报告不一致的情况。

## 7. 各 agent 注意事项

| agent | 用户级目录 | 注意 |
|---|---|---|
| Claude Code | `~/.claude/skills` | 跟随软链接。`synced/` 是 Claude 自己同步的目录，不纳管 |
| Codex | `~/.codex/skills` | 本机实际在加载这里；官方文档另有 `~/.agents/skills`，**不要两处都链**，否则同名重复。`.system/` 是 Codex 自带，不纳管 |
| WorkBuddy | `~/.workbuddy/skills` | 目录里的 `.*migration.json`、`_bm_skillid_migration.json` 是 WorkBuddy 自己的文件，不动；WorkBuddy 从 skillhub 装的 skill 由它自己管理，不纳管 |
| OpenClaw | `~/.openclaw/skills` | `~/.openclaw/plugin-skills`、`workspace/` 下的 skill 由 OpenClaw 管理，不纳管 |
| Kairos | 计划 `~/.kairos/skills` | 接入要求：读取一个目录下的 `<name>/SKILL.md`（遵循通用 Agent Skills 格式：frontmatter `name` + `description`，正文按需加载），**必须跟随软链接**。接入后在清单里加一行即可 |

- **项目级 skill**（各项目里的 `.claude/skills`、`.agents/skills`）不归 `skl` 管，它们跟着项目走。
- 加新 agent：只需在 `[agents]` 加一行路径，然后在 `[skills]` 给需要的 skill 加上它，`skl sync`。
- 软链接是绝对路径。换电脑时用户名不同也没关系，`skl sync` 会按新路径重建。

## 8. 换新电脑

```bash
# 1. 装工具
cargo install --git https://github.com/Austinenna/skilldock skl

# 2. 拉 skill 仓库（私有）
git clone <私有仓库地址> ~/Projects/skills

# 3. 指定根目录（只需一次）
skl init --root ~/Projects/skills

# 4. 生成所有 agent 的链接
skl sync
skl doctor
```

需要另外处理的：

- **带依赖的 skill**：git 不含 `node_modules`，clone 后开发区和发布区都缺依赖。先在开发区装好依赖（如 `cd dev/wechat-styles && npm ci`），然后 `skl publish <name>` 再发布一次。目前只有 `wechat-styles` 属于这种情况；`skl doctor` 以后可以加一项检查，提示“有 `package.json` 但没有 `node_modules`”。
- skill 用到的凭证、`.env`、网关 Token 属于机器本身，不在仓库里，按各 skill 的 setup 文档重新配置。

## 9. 让 agent 也懂这套流程

规则写在这里，但 agent 不会自己来读。计划做一个 `skill-manager` skill（在 `dev/` 开发、按本流程测试和发布），内容是：skill 放哪、怎么用 `skl`、哪些操作需要用户同意。发布给所有 agent 后，任何 agent 在任何电脑上都按同一套规则工作，不依赖某一家的对话记忆。

`~/.codex/AGENTS.md` 里的“Skill 开发与安装”一节需要同步改成本流程（路径改为 `dev/`、全局安装改为 `skl publish`）。

## 10. SkillDock GUI 改造

### 代码结构

```
skilldock/
├── crates/skl/        # 核心库 + skl 命令行（新）
├── src-tauri/         # GUI 后端：改为调用 crates/skl 的库函数
└── src/               # GUI 前端
```

### 功能取舍

| 模块 | 功能 | 处理 |
|---|---|---|
| AI 分析 | AI 摘要、缓存、全部重新生成、连接测试（`ai.rs` 731 行） | **删除** |
| | API Key 存 Keychain（`keyring` 依赖） | **删除** |
| | 设置页 provider / endpoint / model；`config.rs` 中的 `ai_*` 字段 | **删除** |
| 初始化迁移 | 扫描、预览、fingerprint、备份、journal、回滚（`repository_init.rs` 1902 行 + `InitRepositoryModal`） | **删除**，由 `doctor` + `adopt` + `sync` 替代 |
| Skill 管理 | 扫描、frontmatter 解析、详情、文件列表、路径安全校验 | 保留，逻辑移入 `crates/skl` |
| | Import（直接写进中央库） | 改为 `new`，建在 `dev/` |
| | Delete | 改为 `unpublish`；`dev/` 删除需二次确认 |
| 路由 | 每个平台一个开关，直接建/删链接 | 改为编辑清单 + `sync` |
| | 写死的 13 个平台 | 删除，读清单 `[agents]` |
| 文件监听 | 递归监听中央库和所有平台目录 | 改为只监听 `skills.toml`、`dev/`、`released/` 顶层 |
| 界面 | 三栏、侧栏计数、搜索、列表/网格、密度、三套主题、Toast | 保留；`App.tsx`（1762 行）拆分文件 |
| 新增 | — | 每个 skill 显示状态（开发中 / 已发布 / 有未发布改动）；“发布”按钮；doctor 面板 |
| 打包 | `.app` + `.dmg` 发布脚本 | 保留 |

**状态**：2026-10-25 已按上表完成改造。界面上的“发布”需要连点两次确认，点击本身就等同于用户授权。如果一个月内都不需要打开 GUI，可把 GUI 部分移入 `~/Projects/archive`，只保留 `crates/skl`。

**未做**：删除开发区 skill（目前只能手动删）；`skl test` 沙盒入口；在界面里编辑 `skills.toml` 的 `[agents]`。

## 11. 本机记录

作者本机的迁移记录、skill 清单和待办放在私有的 skills 仓库（`docs/migration-2026-10-25.md`），不在公开仓库中。
