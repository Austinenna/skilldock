# SkillLoom 实施报告

## 任务 1：添加 Skill ID 校验

提交目标：`fix: validate skill ids`

修改文件：
- `src-tauri/src/skills.rs`
- `src-tauri/src/routes.rs`

修改内容：
- 在 Rust 后端新增共享的 `validate_skill_id(id: &str) -> Result<()>` 校验逻辑。
- 将 skill id 限制为简单目录名，只允许小写 ASCII 字母、数字、`.`、`_` 和 `-`。
- 拒绝空 id、`.`、`..`、路径分隔符、NUL 字节、大写字母、空格和其他不支持的字符。
- 在 central 扫描、导入归一化、路由添加/移除和删除操作中复用同一个校验器。
- 为合法 id、空值/dot 路径、路径穿越、分隔符和不支持字符补充单元测试。

验证：
- `cargo fmt`
- `cargo test`
- 结果：4 个 Rust 测试通过。

## 任务 2：显式加固路径包含关系

提交目标：`fix: harden route path checks`

修改文件：
- `src-tauri/src/skills.rs`
- `src-tauri/src/routes.rs`

修改内容：
- 新增 `central_skill_path`，确保 central skill 路径只会在 skill id 校验通过后构造。
- 新增 `existing_central_skill_paths`，对已存在的 skill 目录做 canonicalize，并拒绝解析到 central skills 目录之外的路径。
- 新增 `link_points_to_path` 比较 symlink 目标：能 canonicalize 时使用 canonical 路径，路径中包含缺失组件时使用词法归一化。
- 在路由添加/移除、路由扫描、导入和删除中复用加固后的 central 路径 helper。
- 增加一个单元测试，覆盖 symlink 目标路径需要归一化但无法 canonicalize 的场景。

验证：
- `rustfmt src/skills.rs src/routes.rs`
- `cargo test`
- `cargo check`
- 结果：5 个 Rust 测试通过；Rust check 通过。

## 任务 3：添加路由行为测试

提交目标：`test: cover route symlink behavior`

修改文件：
- `src-tauri/src/routes.rs`

修改内容：
- 将路由添加/移除的核心逻辑抽成可传入 central 和 platform root 的 helper。
- Tauri command 仍然使用配置的平台列表，同时让文件系统行为可以直接测试。
- 增加测试：目标 symlink 已指向 central 时，添加路由保持幂等。
- 增加测试：目标是真目录或指向其他位置的 symlink 时，添加路由会报冲突。
- 增加测试：移除路由拒绝删除真实目录，并会删除指向 central 的 symlink。

验证：
- `rustfmt src/routes.rs`
- `cargo test`
- `cargo check`
- 结果：10 个 Rust 测试通过；Rust check 通过。

## 任务 4：正确解析 SKILL.md Frontmatter

提交目标：`feat: parse skill metadata`

修改文件：
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/src/skills.rs`

修改内容：
- 引入 `serde_yaml`，用于真实解析 SKILL.md frontmatter。
- 增加 `name`、`description`、`version` 和 `tags` 元数据解析。
- 在 frontmatter 缺失、格式错误或字段不完整时保留安全 fallback。
- 用 `read_skill_metadata` 替换旧的仅描述扫描逻辑。
- 增加解析器测试，覆盖正常 frontmatter、带引号描述、字符串/列表 tags、正文 fallback、字段缺失和 YAML 格式错误。

验证：
- `rustfmt src/skills.rs`
- `cargo test`
- `cargo check`
- `pnpm build`
- 结果：15 个 Rust 测试通过；Rust check 通过；前端生产构建通过。

## 任务 5：添加 Skill 详情命令

提交目标：`feat: show skill details`

修改文件：
- `src-tauri/src/skills.rs`
- `src-tauri/src/main.rs`
- `src/ipc.ts`
- `src/types.ts`
- `src/App.tsx`

修改内容：
- 新增后端响应类型 `SkillDetail` 和 `SkillFile`。
- 新增 `get_skill_detail(id)` Tauri command，并复用现有 skill id 校验和 central 路径包含检查。
- 返回解析后的 `Skill`、原始 `SKILL.md` 内容、source path，以及直接文件条目的 kind、size 和 modified time。
- 为 skill 详情补充前端 IPC 封装和 TypeScript 类型。
- 选中 skill 时加载详情数据，并用真实 source path 和文件表替换 Files 区块占位内容。

验证：
- `rustfmt src/skills.rs src/main.rs`
- `cargo test`
- `cargo check`
- `pnpm build`
- 结果：15 个 Rust 测试通过；Rust check 通过；前端生产构建通过。

## 任务 6：替换 Alert 式错误提示

提交目标：`feat: improve app error feedback`

修改文件：
- `src/App.tsx`

修改内容：
- 新增轻量的应用级 notice/toast 组件。
- 将阻塞式 `alert(...)` 调用替换为非阻塞通知。
- 为扫描、路由添加/移除、导入、删除、配置保存和路由冲突添加操作级错误消息。
- 启动失败仍保留原有的全屏启动错误页。

验证：
- `rg -n "alert\\(" src/App.tsx`
- `pnpm build`
- 结果：没有剩余 `alert(...)` 调用；前端生产构建通过。

## 任务 7：添加 Pending 状态和手动刷新

提交目标：`feat: add refresh and pending states`

修改文件：
- `src/App.tsx`

修改内容：
- 为 skill 扫描、路由开关、skill 删除和 skill 导入添加 pending 状态。
- 在列表头部添加手动 Refresh 按钮。
- 操作进行中禁用受影响按钮。
- 刷新后如果原选中 skill 仍存在，则保留选择；否则选择第一个可用 skill。

验证：
- `pnpm build`
- 结果：前端生产构建通过。

## 任务 8：添加文件监听

提交目标：`feat: refresh skills from file changes`

修改文件：
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/src/error.rs`
- `src-tauri/src/main.rs`
- `src-tauri/src/watcher.rs`
- `src/App.tsx`

修改内容：
- 引入 `notify`，并新增一个后端 watcher，通过 Tauri managed state 保持生命周期。
- 启动时监听 central skills 目录和当前配置中可见平台的 skill 目录。
- 对文件系统事件做防抖，然后发出 `skills-changed` 事件。
- 前端监听 `skills-changed`，收到事件后复用现有扫描流程刷新 skills。
- 保留手动 Refresh 按钮作为兜底。

验证：
- `rustfmt src/watcher.rs src/error.rs`
- `cargo check`
- `cargo test`
- `pnpm build`
- 结果：Rust check 通过；15 个 Rust 测试通过；前端生产构建通过。

## 任务 9：清理文档和路线图

提交目标：`docs: update project roadmap`

修改文件：
- `README.md`
- `DEVELOPMENT.md`

修改内容：
- 更新 README，让它描述当前 MVP 状态，而不是早期原型状态。
- 补充当前限制、开发命令和 GitHub/CI 流程说明。
- 更新项目结构文档，加入 `watcher.rs`、`docs/plans` 和 `docs/reports`。
- 更新 DEVELOPMENT 路线图勾选状态，标记脚手架、扫描、元数据、路由、导入/删除、watcher 和配置工作已完成。
- 在 DEVELOPMENT 中补充当前限制和本地验证命令。

验证：
- `pnpm build`
- `cargo check`
- 结果：前端生产构建通过；Rust check 通过。

## 任务 10：添加密钥存储

提交目标：`feat: store ai api key securely`

修改文件：
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/src/ai.rs`
- `src-tauri/src/error.rs`
- `src-tauri/src/main.rs`
- `src/ipc.ts`
- `src/types.ts`
- `src/App.tsx`

修改内容：
- 引入 `keyring`，并新增后端 `ai` 模块用于安全存储 API key。
- 实现 `get_api_key_status`、`set_api_key` 和 `clear_api_key`。
- 前端只接收 configured/not-configured 状态；secret 永远不会返回到 UI。
- 在 Settings UI 中添加保存和清除 key 的入口，并配套 pending 状态和通知。

验证：
- `rustfmt src/ai.rs src/error.rs`
- `cargo check`
- `cargo test`
- `pnpm build`
- 结果：Rust check 通过；15 个 Rust 测试通过；前端生产构建通过。

## 任务 11：生成并缓存摘要

提交目标：`feat: generate cached skill summaries`

修改文件：
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/src/ai.rs`
- `src-tauri/src/error.rs`
- `src-tauri/src/main.rs`
- `src-tauri/src/skills.rs`
- `src/ipc.ts`
- `src/App.tsx`

修改内容：
- 新增 `generate_summary(skill_id, force)` Tauri command，并通过前端 IPC 层接入。
- 为 `SKILL.md` 增加 SHA-256 内容 hash，skill 内容变化时缓存会自动失效。
- 使用 macOS 自带 `/usr/bin/sqlite3` 新增本地 `ai_summary` 缓存，避免下载新的数据库 crate。
- 保持 API 生成可选：未配置 API key 时，命令会降级返回解析出的 skill 描述。
- 后续如果配置了 key，后端可以通过 macOS `curl` 请求 Anthropic 摘要；本步骤未触发 live 请求。
- 删除 skill 时会清理对应的缓存摘要。
- 用加载、错误、fallback、缓存摘要和重新生成状态替换 AI summary 占位面板。

验证：
- `rustfmt src-tauri/src/ai.rs src-tauri/src/error.rs src-tauri/src/skills.rs`
- `cargo check --offline`
- `cargo test --offline`
- `pnpm build`
- 结果：离线 Rust check 通过；15 个 Rust 测试通过；前端生产构建通过。
- 备注：live API 生成和依赖下载已按要求跳过。

## 任务 12：准备 macOS 构建

提交目标：`build: prepare macos packaging`

修改文件：
- `src-tauri/tauri.conf.json`
- `README.md`
- `DEVELOPMENT.md`

修改内容：
- 开启 Tauri macOS `.app` 目标的 bundle 生成。
- 保留 `src-tauri/icons/` 中已有的真实图标集，并沿用现有 bundle icon 配置。
- 将本地 unsigned build 与正式签名/公证 release build 分开记录。
- 更新 README 和 DEVELOPMENT，反映已完成的 Keychain/API 摘要工作、当前本地 bundle 状态和剩余 release 缺口。
- 在 Apple Developer 凭证准备好之前，暂缓 DMG、签名、公证和 release workflow。

验证：
- `pnpm tauri build`
- `git diff --check`
- 结果：前端构建、release 二进制构建和 unsigned `.app` bundle 均通过。
- Bundle 输出：`src-tauri/target/release/bundle/macos/SkillLoom.app`（4.3M）。
- 备注：第一次 all-target bundle 尝试已生成 `.app`，但在 DMG 脚本阶段失败，因此当前 bundle target 有意收窄为 `.app`。

## 任务 13：调整 SQLite 缓存实现

提交目标：`refactor: simplify sqlite cache`

修改文件：
- `src-tauri/src/ai.rs`
- `DEVELOPMENT.md`
- `docs/reports/2026-05-28-implementation-report.md`

修改内容：
- 移除 `ai.rs` 中手写的 `sqlite3` unsafe FFI 包装。
- 保留 `cache.db` 和 `ai_summary` 表结构，继续用 SQLite 做 AI 摘要缓存。
- 改为通过 macOS 自带 `/usr/bin/sqlite3` 执行极薄的本地缓存读写逻辑，符合当前“mac 优先”的目标。
- 增加 SQL 字符串字面量转义，避免 skill id、hash、summary 写入 SQL 时破坏语句。
- 增加 `sql_literal` 单元测试，覆盖单引号转义和 NUL 字节拒绝。
- 更新 DEVELOPMENT 中的本地缓存 DB 说明。

验证：
- `which sqlite3`
- `sqlite3 -version`
- `rustfmt src-tauri/src/ai.rs`
- `cargo check --offline`
- `cargo test --offline`
- `pnpm build`
- `pnpm tauri build`
- 结果：`sqlite3` 可用；离线 Rust check 通过；17 个 Rust 测试通过；前端生产构建通过；macOS unsigned `.app` 打包通过。
