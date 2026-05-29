# SkillLoom 开发文档

> 把当前的 HTML/React 原型，做成一个能在 macOS 上跑的真应用，并为后续扩展到 Windows/Linux 留好接口。

---

## 1. 项目目标

SkillLoom 是一个**集中式 Skill 管理器**：

- 所有 skill 真实存放在 **中央目录** `~/.skillloom/skills/`
  （**注意**：刻意避开 `~/.agents/skills/`——那是 Codex CLI 自己的目录，作为中央仓库会污染 Codex 的真实文件，而且 Codex 的路由 symlink 会指回自己，形成循环）
- 各 AI 工具（Claude Code、Codex CLI、OpenClaw 等）的 skill 目录里只放**符号链接**，指向中央目录里的真实条目
- 用户在 SkillLoom 里通过开关勾选「这个 skill 路由到哪些平台」，本质就是新建 / 删除对应的 symlink
- 每个 skill 提供一段 AI 自动生成的摘要，帮助用户快速理解作用
- 当前阶段聚焦 macOS，后续再扩展到其他平台

---

## 2. 当前实现状态

```
SkillLoom/
├── prototype/          # 早期 HTML/React CDN 原型，留作设计参考
├── src/                # Vite + React + TypeScript 前端
├── src-tauri/          # Tauri 2 + Rust 后端
├── docs/plans/         # roadmap 执行计划
└── docs/reports/       # 每步修改报告
```

- 前端已经迁移到 Vite + React 18 + TypeScript。
- 后端已经实现真实文件系统扫描、SKILL.md metadata 解析、symlink 路由、导入、删除、详情读取、配置持久化、文件监听、Keychain API key 状态和 AI 摘要缓存。
- 路由和删除路径已经做 skill id 校验、canonical path containment、真实目录拒删、冲突 symlink 保护。
- 前端已经有非阻塞 notice、pending 状态、手动 Refresh、`skills-changed` 自动刷新、API key 设置入口和 AI Summary 状态。
- macOS `.app` bundle 已开启，可跑本地 unsigned build。

**当前还差三件事：**

1. **真实 API 凭证验证**（保存用户 key 后跑一次 live summary）
2. **可分发的 macOS .app**（Developer ID 签名、notarization）
3. **CI / release workflow**（PR 验证、tag 打包）

---

## 3. 技术选型

### 3.1 框架：**Tauri 2.x** ✅

| 候选 | 包体积 | 跨平台 | 文件系统 API | 上手难度 | 推荐度 |
|---|---|---|---|---|---|
| **Tauri** | ~10 MB | macOS / Win / Linux | Rust 后端，原生 | 中（要学一点 Rust） | ★★★★★ |
| Electron | ~150 MB | 同上 | Node fs | 低 | ★★★ |
| Swift / SwiftUI | ~5 MB | **仅 Apple 全家桶** | 原生最佳 | 高，且 UI 要重写 | ★★（被锁死） |
| Wails (Go) | ~10 MB | 同 Tauri | Go std | 中 | ★★★★（生态弱于 Tauri） |

**为什么选 Tauri：**

- 当前 React UI 几乎能**原样搬过去**，配色/排版/交互一行不用改
- Rust 写文件系统操作（symlink、watcher、权限处理）类型安全、性能高，比 Node 稳
- 跨平台从 day 1 就具备，不会等到「macOS 做好了再纠结怎么搬 Win」
- 自带 codesigning、notarization、auto-updater 工作流
- 包体积小（~10 MB vs Electron ~150 MB），冷启动快

> 如果你完全不想碰 Rust，可以退而求其次用 Electron + Node。但 symlink/权限/notarization 的坑 Tauri 帮你踩过了大半，长期 ROI 更高。

### 3.2 前端

| 层 | 选型 | 说明 |
|---|---|---|
| 构建工具 | **Vite 5** | 替代 CDN + Babel standalone；HMR 体验好 |
| 框架 | **React 18** | 原型已是 React，零迁移 |
| 语言 | **TypeScript 5** | 真应用必须上 TS，对 IPC 接口的类型安全尤其重要 |
| 状态 | **Zustand**（轻量全局） + React state（局部） | 不用 Redux，复杂度不值得 |
| 异步 | **TanStack Query**（@tanstack/react-query） | 包装 Tauri command 调用，自动缓存、重试、失效 |
| 样式 | **保留 inline style**（短期）→ **CSS Modules / Tailwind**（中期） | 原型用的就是 inline style，先不动 |
| 图标 | **lucide-react** | 替换原型里的 emoji/字符占位 |

### 3.3 后端（Tauri Rust）

| 用途 | crate |
|---|---|
| Tauri 核心 | `tauri` 2.x |
| 文件操作 | `std::fs` + `std::os::unix::fs::symlink` |
| 文件监听 | `notify` |
| 序列化 | `serde`, `serde_json` |
| HTTP（调 Claude API） | macOS `/usr/bin/curl`（避免额外下载依赖；有 key 时才触发） |
| 本地缓存 DB | macOS `/usr/bin/sqlite3` + `sha2` content hash |
| 安全存 API key | `keyring`（走 macOS Keychain） |
| 错误处理 | `thiserror` + `anyhow` |
| 异步运行时 | `tokio`（Tauri 自带） |
| 日志 | `tracing` + `tracing-subscriber` |

---

## 4. 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                    SkillLoom.app                        │
│  ┌──────────────────────────┐                           │
│  │   Frontend (Webview)     │                           │
│  │   Vite + React + TS      │                           │
│  │                          │                           │
│  │   - Sidebar / List /     │                           │
│  │     Detail / Settings    │                           │
│  │   - Zustand store        │                           │
│  │   - TanStack Query       │                           │
│  └────────────┬─────────────┘                           │
│               │  invoke(command, args)                  │
│               │  / listen(event)                        │
│  ┌────────────▼─────────────┐                           │
│  │   Backend (Rust)         │                           │
│  │   Tauri Commands         │                           │
│  │                          │                           │
│  │   ┌────────────────┐     │                           │
│  │   │ skill_scanner  │ ───▶│ ~/.skillloom/skills/         │
│  │   │ route_manager  │ ───▶│ ~/.claude/skills/         │
│  │   │ ai_summarizer  │ ───▶│ ~/.openclaw/skills/  ...  │
│  │   │ fs_watcher     │     │                           │
│  │   │ config_store   │ ───▶│ ~/Library/App Support/    │
│  │   └────────────────┘     │     SkillLoom/            │
│  └──────────────────────────┘                           │
└─────────────────────────────────────────────────────────┘
                     │
                     │ HTTPS
                     ▼
            自定义 AI endpoint (摘要生成)
```

**核心设计原则：**

- **文件系统是 single source of truth**，App 不维护自己的 skill 列表，每次需要就 scan
- **App 自己的状态只存偏好**（主题、隐藏平台、AI provider/endpoint/model、AI 摘要缓存）
- **API key 只进 Keychain**，不写入 config.json
- **Symlink 是路由的唯一实现**，没有任何"软配置文件"决定路由，避免不一致

---

## 5. 数据模型

### 5.1 Skill（运行时类型）

```typescript
// src/types/skill.ts
export interface Skill {
  id: string;          // 目录名，e.g. "pdf-extract"
  title: string;       // SKILL.md frontmatter.name
  tagline: string;     // SKILL.md frontmatter.description（截到 1 行）
  version: string;     // frontmatter.version || "0.0.0"
  files: number;       // fs::read_dir 出来的文件数
  size: string;        // 人读尺寸，"48 KB"
  updated: string;     // 最后修改时间（相对，e.g. "2 days ago"）
  tags: string[];      // frontmatter.tags
  routes: PlatformId[];// 哪些平台 skills/ 下有 symlink 指过来
  ai?: string;         // AI 摘要（从缓存读，没有则空）
  sourcePath: string;  // 绝对路径，e.g. "~/.skilldock/skills/pdf-extract"
}
```

### 5.2 Platform

```typescript
export interface Platform {
  id: PlatformId;
  name: string;
  short: string;
  path: string;        // ~/.claude/skills/ 这种带 ~ 的形式
  group: 'Core' | 'Coding' | 'Lobster';
  isHub?: boolean;
  visible: boolean;    // 用户在 Settings 里的可见性
}
```

平台定义是**静态配置**，硬编码在 Rust 端（`platforms.rs`），前端通过 `list_platforms` 命令拿到。

### 5.3 App 偏好（持久化在磁盘）

存放位置：`~/Library/Application Support/SkillLoom/config.json`

```json
{
  "palette": "cool",
  "density": "comfortable",
  "view": "list",
  "hiddenPlatforms": ["cursor", "gemini", "..."],
  "aiProvider": "anthropic",
  "aiEndpoint": "https://api.minimaxi.com/anthropic/v1/messages",
  "aiModel": "MiniMax-M2.7"
}
```

API key 不存这里，走 Keychain（见 §8.3）。

### 5.4 AI 摘要缓存（SQLite）

存放位置：`~/Library/Application Support/SkillLoom/cache.db`

```sql
CREATE TABLE ai_summary (
  skill_id      TEXT NOT NULL,
  content_hash  TEXT NOT NULL,     -- SKILL.md 内容的 SHA256，变了就重新生成
  summary       TEXT NOT NULL,
  model         TEXT NOT NULL,
  generated_at  INTEGER NOT NULL,  -- Unix epoch
  PRIMARY KEY (skill_id, content_hash)
);
```

读取缓存时会额外按 `model` 字段过滤，字段内容包含 provider、model 和 endpoint hash（例如 `anthropic:MiniMax-M2.7:<hash>`），避免切换 provider/model/endpoint 后误用旧摘要。

---

## 6. 后端命令（IPC 接口）

所有命令都是 `#[tauri::command] async fn` 形式，错误用统一的 `AppError` 枚举。

### 6.1 扫描类

```rust
list_platforms() -> Vec<Platform>
scan_skills() -> Vec<Skill>            // 扫 central，并对每个平台 readdir 推 routes
get_skill_detail(id: String) -> SkillDetail   // 含完整 SKILL.md 内容
```

### 6.2 路由类

```rust
add_route(skill_id: String, platform_id: String) -> Result<()>
remove_route(skill_id: String, platform_id: String) -> Result<()>
// 内部实现：
//   add:    symlink(~/.skillloom/skills/<id>, ~/.claude/skills/<id>)
//   remove: 先确认 ~/.claude/skills/<id> 是 symlink 且指向 central，再 unlink
```

**安全检查（必须）：**

- 创建 symlink 前确保目标在 `~/.skillloom/skills/` 下，防止做出指向系统目录的链接
- 删除前必须先 `symlink_metadata` 判断是 symlink，**绝不能 rm 真目录**
- 平台目标目录不存在时**自动创建**（用户可能从没装过那个工具）

### 6.3 Skill 增删

```rust
import_skill(name: String, description: String) -> Result<Skill>
delete_skill(id: String) -> Result<()>
// delete 实现：
//   1. 遍历所有平台 skills/ 目录，删掉指向该 skill 的 symlink
//   2. 删除 ~/.skillloom/skills/<id> 真目录
//   3. 删除 AI 摘要缓存行
```

### 6.4 AI 摘要

```rust
generate_summary(skill_id: String, force: bool) -> Result<String>
// force=false 时优先读当前 provider/model/endpoint 对应缓存
test_ai_config() -> Result<AiTestResult>
// 使用当前 provider/endpoint/model/API key 发送一条短测试请求
// provider=anthropic 时发 Anthropic Messages 请求，使用 x-api-key
// provider=chat 时发 Chat Completions 请求，使用 api-key
```

### 6.5 文件监听（推事件给前端）

```rust
// 不是 command，是后台任务
// 启动时跑 notify::Watcher，监听：
//   ~/.skillloom/skills/
//   每个 visible 平台的 skills/
// 变化时 emit "skills-changed" 事件，前端 listen 后 invalidate React Query
```

### 6.6 配置

```rust
get_config() -> Config
update_config(patch: ConfigPatch) -> Result<Config>
get_api_key() -> Option<String>     // 从 Keychain 读
set_api_key(key: String) -> Result<()>
// config.json 只保存 aiProvider / aiEndpoint / aiModel，不保存 API key
// Settings 中 endpoint/model 编辑后自动保存；API key 只显示已保存状态，不回显内容
```

---

## 7. 前端改造

### 7.1 项目脚手架

```bash
# 一次性初始化（在临时目录）
pnpm create tauri-app skillloom
# 选 React + TypeScript + pnpm

# 然后把当前 src/app.jsx 的组件拆进新工程
```

迁移后的目录大致：

```
SkillLoom/
├── src/                         # 前端
│   ├── main.tsx
│   ├── App.tsx
│   ├── components/
│   │   ├── Sidebar.tsx
│   │   ├── SkillList.tsx
│   │   ├── SkillDetail.tsx
│   │   ├── SettingsPane.tsx
│   │   └── ImportModal.tsx
│   ├── store/
│   │   ├── usePreferences.ts    # Zustand
│   │   └── useSkills.ts         # TanStack Query wrappers
│   ├── ipc/
│   │   └── commands.ts          # invoke 的薄封装，给所有命令上类型
│   ├── styles/
│   │   └── palettes.ts          # 原 palettes.js
│   └── types/
│       └── index.ts
├── src-tauri/                   # 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs
│       ├── commands/
│       │   ├── skills.rs
│       │   ├── routes.rs
│       │   ├── ai.rs
│       │   └── config.rs
│       ├── fs/
│       │   ├── scanner.rs
│       │   └── symlink.rs
│       ├── watcher.rs
│       ├── platforms.rs
│       └── error.rs
└── package.json
```

### 7.2 IPC 封装示例

```typescript
// src/ipc/commands.ts
import { invoke } from '@tauri-apps/api/core';
import type { Skill, Platform, Config } from '@/types';

export const api = {
  listPlatforms: () => invoke<Platform[]>('list_platforms'),
  scanSkills:    () => invoke<Skill[]>('scan_skills'),
  addRoute:      (skillId: string, platformId: string) =>
    invoke<void>('add_route', { skillId, platformId }),
  removeRoute:   (skillId: string, platformId: string) =>
    invoke<void>('remove_route', { skillId, platformId }),
  importSkill:   (name: string, description: string) =>
    invoke<Skill>('import_skill', { name, description }),
  deleteSkill:   (id: string) => invoke<void>('delete_skill', { id }),
  generateSummary: (id: string, force = false) =>
    invoke<string>('generate_summary', { skillId: id, force }),
  getConfig:     () => invoke<Config>('get_config'),
  updateConfig:  (patch: Partial<Config>) =>
    invoke<Config>('update_config', { patch }),
};
```

### 7.3 React Query 用法

```typescript
// src/store/useSkills.ts
export function useSkills() {
  return useQuery({
    queryKey: ['skills'],
    queryFn: api.scanSkills,
    staleTime: 30_000,
  });
}

export function useToggleRoute() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ skillId, platformId, on }: ToggleArgs) =>
      on ? api.addRoute(skillId, platformId)
         : api.removeRoute(skillId, platformId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['skills'] }),
  });
}
```

### 7.4 文件变化驱动刷新

```typescript
// src/App.tsx
useEffect(() => {
  const unlisten = listen('skills-changed', () => {
    queryClient.invalidateQueries({ queryKey: ['skills'] });
  });
  return () => { unlisten.then((fn) => fn()); };
}, []);
```

---

## 8. macOS 专题

### 8.1 路径展开

`~/.skillloom/skills/` 这类带 `~` 的路径在 Rust 里要展开：

```rust
let home = dirs::home_dir().ok_or(AppError::NoHomeDir)?;
let central = home.join(".skillloom").join("skills");
```

用 `dirs` crate，不要手 parse `$HOME`。

### 8.2 应用数据目录

```rust
// Tauri 提供
let app_data = app.path().app_data_dir()?;
// macOS: ~/Library/Application Support/com.skillloom.app/
```

### 8.3 API Key → Keychain

```rust
use keyring::Entry;

fn set_key(key: &str) -> Result<()> {
    Entry::new("com.skillloom.app", "anthropic_api_key")?.set_password(key)?;
    Ok(())
}
```

不要存到 config.json，磁盘明文 = 安全审查不过。

### 8.4 沙箱

**不开启沙箱**。原因：要读写 `~/.claude/`、`~/.openclaw/` 等任意 dotfile 目录，sandbox 下需要 user-selected file 权限或 Group Container entitlements，体验差。

代价：**不能上 Mac App Store**，只能从官网/GitHub Releases 分发。对开发者工具来说完全可接受。

### 8.5 本地 unsigned build

当前已开启 Tauri `.app` bundle，可在本机验证 unsigned app 产物：

```bash
pnpm tauri build
```

输出目录：

```text
src-tauri/target/release/bundle/
```

这个构建没有 Developer ID 签名，也没有 notarization，适合自己机器 smoke test；直接发给别人时可能触发 Gatekeeper，需要对方手动右键打开。DMG 生成留到签名/公证发布流程一起处理。

### 8.6 Codesigning + Notarization

每次发版的硬性流程，在 GitHub Actions 里跑：

1. 准备 **Developer ID Application** 证书（年费 $99 的 Apple Developer Program）
2. `tauri build --target universal-apple-darwin` 出 arm64 + x86_64 通用二进制
3. 用 `tauri-action` 自动签名 + notarize（apple-id / app-specific-password / team-id 走环境变量）
4. 产物：`.dmg` + `.app.tar.gz`（前者给人下载，后者给 updater）

不做这步：用户首次打开会撞 Gatekeeper 提示「无法验证开发者」，几乎没人会去走「右键 → 打开」的绕路。

### 8.7 自动更新

Tauri 内置 updater：

- 后端在 `Cargo.toml` 启 `tauri = { features = ["updater"] }`
- 配 `tauri.conf.json` 里的 `updater.endpoints` 指向 `https://releases.skillloom.app/{{target}}/{{current_version}}`
- 该 endpoint 返回签名后的 JSON（公钥校验，防中间人）
- GitHub Releases + 一个简单的 Cloudflare Worker / Vercel function 转发即可

---

## 9. 开发路线图

### Phase 0 — 脚手架（0.5 天）
- [x] Tauri 2 + Vite + React + TypeScript 工程已落地
- [x] 原型保留在 `prototype/`，真实前端迁入 `src/`
- [ ] CI：lint + typecheck + `cargo check`

### Phase 1 — 只读 MVP（2 天）
- [x] `list_platforms` / `scan_skills` 跑通
- [x] 真实读取 SKILL.md frontmatter（Rust `serde_yaml`）
- [x] 列表 / 详情显示真实数据
- [x] **里程碑**：能跑起来看自己 `~/.skillloom/skills/` 真实内容

### Phase 2 — Symlink 路由（1.5 天）
- [x] `add_route` / `remove_route` 实现 + 单元测试
- [x] 前端 Toggle 接上 Tauri IPC
- [x] 错误处理（真实目录冲突、指向别处的 symlink、hub 路由拒绝）
- [x] **里程碑**：勾选开关会创建/删除平台 symlink

### Phase 3 — Skill 增删（1 天）
- [x] `import_skill`：建中央目录 + 默认 SKILL.md 模板
- [x] `delete_skill`：先扫所有 symlink 删掉，再删真目录
- [x] 删除 AI 摘要缓存行
- [x] **里程碑**：Import / Delete 按钮真能干活

### Phase 4 — AI 摘要（1.5 天）
- [x] Settings 里加 API Key 输入框，存 Keychain
- [x] Settings 里配置 provider、endpoint 和 model，编辑后自动保存
- [x] Settings 里提供 AI 连接测试按钮
- [x] `generate_summary` 按配置调用 Anthropic Messages 或 Chat Completions 兼容端点
- [x] SQLite 缓存按 content_hash + provider/model/endpoint 命中
- [x] 自动摘要无 key 时降级到 SKILL.md 原始描述；手动 Regenerate 无 key 时给出明确错误
- [ ] **里程碑**：配置真实 API key 后验证每个 skill 都有像样的摘要

### Phase 5 — 文件监听（1 天）
- [x] `notify::Watcher` 跑在后台 thread
- [x] 防抖 350ms，emit `skills-changed`
- [x] 前端收到事件后刷新 skills
- [x] **里程碑**：外部文件变化会触发 UI 刷新

### Phase 6 — 偏好持久化（0.5 天）
- [x] config.json 读写
- [x] 主题/密度/隐藏平台走 backend config
- [x] **里程碑**：所有设置重启后保留

### Phase 7 — 打包分发（2 天，含跑通 CI）
- [x] 启用本地 unsigned Tauri `.app` bundle
- [ ] Apple Developer ID 证书申请（如果还没有）
- [ ] `tauri-action` workflow，PR / tag 触发
- [ ] 通用二进制 + DMG + notarization
- [ ] 简单官网（GitHub Pages 即可）放下载链接
- [ ] **里程碑**：朋友下载装 .app，双击能直接跑

### Phase 8 — 自动更新（0.5 天）
- [ ] updater endpoint
- [ ] 应用内"检查更新"按钮

### Phase 9+ — 跨平台扩展
见 §11。

**总计：~10 个工作日到可发布的 v1.0。**

---

## 9.1 当前限制

- AI 摘要的 live API 请求需要用户自行配置 provider、endpoint、model 和 API key；Settings 可测试连接，当前本地验证不提交任何真实密钥。
- `notify` watcher 在启动时读取一次隐藏平台配置；运行中改变可见平台后，手动 Refresh 仍是兜底。
- macOS `.app` bundle 已开启，但当前只适合本机 unsigned build；正式分发还需要签名、notarization 和 DMG/release 流程。
- 还没有 GitHub Actions CI，发布流程需要在 Phase 7 补齐。

## 9.2 本地 GitHub 与开发命令

```bash
git status --short --branch
git log --oneline --decorate -5
git remote -v
```

当前主线分支是 `main`。roadmap 任务按「实现 -> 验证 -> 更新报告 -> 单独提交」推进。

本地验证命令：

```bash
pnpm build
cd src-tauri && cargo check
cd src-tauri && cargo test
```

本地运行桌面窗口：

```bash
pnpm tauri dev
```

后续 CI 建议：

- PR：跑 `pnpm build`、`cargo check`、`cargo test`
- tag：跑 `pnpm tauri build`，并在证书准备好后加入 signing / notarization

---

## 10. 风险与注意事项

### 10.1 Symlink 的坑

| 场景 | 处理 |
|---|---|
| 目标已存在且不是 symlink（用户手动建了真目录） | 提示用户：「Claude 已有同名真实 skill 目录，请手动处理」，绝不覆盖 |
| 目标已存在且是 symlink 但指向别处 | 同上，提示冲突 |
| 中央 skill 被外部删除，平台还留着断链 | 监听器检测到时自动清理断链 |
| 跨 filesystem（中央在 NFS、平台在本地） | symlink 不受影响，但 watcher 可能漏事件，UI 加手动刷新按钮兜底 |

### 10.2 权限

- 不需要 Full Disk Access（只读写 `~` 下的文件）
- 不需要任何特殊 entitlement
- macOS 14+ 对 `~/Library/` 子目录写入也无障碍

### 10.3 性能

- 中央 skill 数过 1000 时全量 scan 会慢，加 mtime 索引或 SQLite 缓存
- AI 摘要必须懒生成 + 缓存，不能在 scan 时同步触发

### 10.4 数据迁移

未来如果改了 SKILL.md schema 或 config.json 字段，加 `schemaVersion`，启动时跑迁移函数。

---

## 11. 跨平台扩展

### 11.1 平台差异

| 维度 | macOS | Linux | Windows |
|---|---|---|---|
| Symlink | 原生 `symlink()` | 原生 `symlink()` | **需要管理员或开发者模式**，否则降级到 hardlink / junction |
| 配置目录 | `~/Library/Application Support/SkillLoom/` | `~/.config/skillloom/` | `%APPDATA%\SkillLoom\` |
| 中央目录默认 | `~/.skillloom/skills/` | 同 | `%USERPROFILE%\.skillloom\skills\` |
| 密钥存储 | Keychain | Secret Service (libsecret) | Credential Manager |
| 代码签名 | Developer ID + notarize | 一般不签 | Authenticode 证书 |

Tauri + `dirs` + `keyring` 三个库已经把上面大部分平台差异封装好，**业务代码不用 cfg**，只在打包阶段切目标。

### 11.2 Windows 的关键决策

Windows 的 symlink 默认要管理员权限。**两个方案：**

1. **要求开发者模式**：Settings → For Developers → Developer Mode 开启后普通用户能建 symlink。文档里加引导。
2. **降级到 junction**（目录的 hardlink）：对 directory 行得通，对 file 不行。SKILL.md 是文件，所以 skill 整个是个目录，可以走 junction，体验等价。

推荐方案 2 + fallback 方案 1，代码上检测到 symlink 失败就尝试 junction。

### 11.3 移动端

不考虑。skill 文件系统映射的概念在 iOS/Android 沙箱里玩不转。

---

## 12. 第一步：怎么开工

```bash
cd ~/Projects/apps/skilldock

# 装 Rust（如果还没装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 装 pnpm
brew install pnpm

# 把当前原型挪到 prototype/ 留作参考
mkdir -p prototype && mv index.html src prototype/

# 起一个新的 Tauri 工程
pnpm create tauri-app .
#   App name: skillloom
#   Window title: SkillLoom
#   Package manager: pnpm
#   UI template: React
#   UI flavor: TypeScript

# 开发模式跑起来
pnpm tauri dev
```

跑通空壳之后，按 §9 Phase 1 开始把原型的组件迁过去。

---

## 13. 决策记录（ADR）

需要记录的关键决策（建议另起 `docs/adr/` 目录）：

1. **ADR-001：选 Tauri 而非 Electron** —— 见 §3.1
2. **ADR-002：Symlink 作为路由唯一实现** —— filesystem 是 truth，避免与配置文件不一致
3. **ADR-003：不上 App Store** —— sandbox 与读取任意 dotfile 不兼容
4. **ADR-004：AI 摘要懒生成 + content_hash 缓存** —— 见 §5.4
5. **ADR-005：API Key 走 Keychain，不存 config.json** —— 见 §8.3

---

## 14. 参考资料

- Tauri 2.x 文档：https://tauri.app/start/
- Anthropic Skills 规范（SKILL.md 结构）：https://docs.claude.com/en/docs/agents-and-tools/agent-skills/overview
- macOS Codesigning + Notarization：https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution
- `tauri-action`（GitHub Actions 一站式打包）：https://github.com/tauri-apps/tauri-action
- `notify` crate（文件监听）：https://docs.rs/notify/
- `keyring` crate（跨平台密钥存储）：https://docs.rs/keyring/
