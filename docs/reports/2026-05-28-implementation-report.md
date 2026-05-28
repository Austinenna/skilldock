# SkillLoom Implementation Report

## Task 1: Add Skill ID Validation

Commit target: `fix: validate skill ids`

Changed files:
- `src-tauri/src/skills.rs`
- `src-tauri/src/routes.rs`

What changed:
- Added shared `validate_skill_id(id: &str) -> Result<()>` validation in the Rust backend.
- Restricted skill ids to simple directory names containing lowercase ASCII letters, digits, `.`, `_`, and `-`.
- Rejected empty ids, `.`, `..`, path separators, NUL bytes, uppercase letters, spaces, and other unsupported characters.
- Reused the shared validator in central scanning, import normalization, route add/remove, and delete operations.
- Added unit tests for valid ids, empty/dot paths, traversal attempts, separators, and unsupported characters.

Verification:
- `cargo fmt`
- `cargo test`
- Result: 4 Rust tests passed.

## Task 2: Make Path Containment Explicit

Commit target: `fix: harden route path checks`

Changed files:
- `src-tauri/src/skills.rs`
- `src-tauri/src/routes.rs`

What changed:
- Added `central_skill_path` so central skill paths are built only after skill id validation.
- Added `existing_central_skill_paths` to canonicalize existing skill directories and reject paths that resolve outside the central skills directory.
- Added symlink target comparison through `link_points_to_path`, using canonical paths when possible and lexical normalization when a path contains missing components.
- Reused the hardened central path helper in route add/remove, route scanning, import, and delete.
- Added a unit test for symlink target comparison when a target path needs normalization but cannot be canonicalized.

Verification:
- `rustfmt src/skills.rs src/routes.rs`
- `cargo test`
- `cargo check`
- Result: 5 Rust tests passed; Rust check passed.

## Task 3: Add Route Behavior Tests

Commit target: `test: cover route symlink behavior`

Changed files:
- `src-tauri/src/routes.rs`

What changed:
- Extracted route add/remove core logic into helpers that accept explicit central and platform root directories.
- Kept Tauri commands wired through the configured platform list while making filesystem behavior directly testable.
- Added tests for idempotent add when a symlink already points to central.
- Added tests for add conflicts when the target is a real directory or a symlink to another location.
- Added tests for remove refusing to delete real directories and deleting symlinks that point to central.

Verification:
- `rustfmt src/routes.rs`
- `cargo test`
- `cargo check`
- Result: 10 Rust tests passed; Rust check passed.

## Task 4: Parse SKILL.md Frontmatter Properly

Commit target: `feat: parse skill metadata`

Changed files:
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/src/skills.rs`

What changed:
- Added `serde_yaml` for real SKILL.md frontmatter parsing.
- Added metadata parsing for `name`, `description`, `version`, and `tags`.
- Preserved safe fallbacks when frontmatter is missing, malformed, or incomplete.
- Replaced the old description-only scanner with `read_skill_metadata`.
- Added parser tests for normal frontmatter, quoted descriptions, string/list tags, body fallback, missing fields, and malformed YAML.

Verification:
- `rustfmt src/skills.rs`
- `cargo test`
- `cargo check`
- `pnpm build`
- Result: 15 Rust tests passed; Rust check passed; frontend production build passed.

## Task 5: Add Skill Detail Command

Commit target: `feat: show skill details`

Changed files:
- `src-tauri/src/skills.rs`
- `src-tauri/src/main.rs`
- `src/ipc.ts`
- `src/types.ts`
- `src/App.tsx`

What changed:
- Added `SkillDetail` and `SkillFile` backend response types.
- Added `get_skill_detail(id)` Tauri command with existing skill id validation and central path containment checks.
- Returned the parsed `Skill`, raw `SKILL.md` content, source path, and direct file entries with kind, size, and modified time.
- Added frontend IPC and TypeScript types for skill details.
- Loaded detail data when a skill is selected and replaced the placeholder Files section with the real source path and file table.

Verification:
- `rustfmt src/skills.rs src/main.rs`
- `cargo test`
- `cargo check`
- `pnpm build`
- Result: 15 Rust tests passed; Rust check passed; frontend production build passed.

## Task 6: Replace Alert-Only Errors

Commit target: `feat: improve app error feedback`

Changed files:
- `src/App.tsx`

What changed:
- Added a small app-level notice/toast component.
- Replaced blocking `alert(...)` calls with non-blocking notices.
- Added operation-specific messages for scan, route add/remove, import, delete, config save, and route conflicts.
- Kept boot failure as the existing full-screen startup error.

Verification:
- `rg -n "alert\\(" src/App.tsx`
- `pnpm build`
- Result: no remaining `alert(...)` calls; frontend production build passed.

## Task 7: Add Pending States And Manual Refresh

Commit target: `feat: add refresh and pending states`

Changed files:
- `src/App.tsx`

What changed:
- Added pending state for skill scanning, route toggles, skill deletion, and skill import.
- Added a manual Refresh button to the list header.
- Disabled affected buttons while their operations are running.
- Preserved the selected skill after refresh when the same id still exists, otherwise selected the first available skill.

Verification:
- `pnpm build`
- Result: frontend production build passed.

## Task 8: Add File Watcher

Commit target: `feat: refresh skills from file changes`

Changed files:
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/src/error.rs`
- `src-tauri/src/main.rs`
- `src-tauri/src/watcher.rs`
- `src/App.tsx`

What changed:
- Added `notify` and a backend watcher that stays alive in Tauri managed state.
- Watched the central skills directory and platform skill directories that are visible in the current config at startup.
- Debounced filesystem events before emitting `skills-changed`.
- Added frontend listener for `skills-changed` that refreshes skills through the existing scan flow.
- Kept the manual Refresh button as a fallback.

Verification:
- `rustfmt src/watcher.rs src/error.rs`
- `cargo check`
- `cargo test`
- `pnpm build`
- Result: Rust check passed; 15 Rust tests passed; frontend production build passed.
