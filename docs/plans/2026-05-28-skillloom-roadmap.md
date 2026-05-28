# SkillLoom Roadmap Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Turn the current first MVP into a safer, test-covered, useful local Skill manager that can be packaged later.

**Architecture:** Keep the filesystem as the source of truth. The Rust backend owns path validation, scanning, symlink routing, and config persistence; the React frontend stays a typed IPC client with focused UI states.

**Tech Stack:** Tauri 2, Rust, React 18, TypeScript, Vite, pnpm, Cargo tests.

---

## Phase 1: Safety And Test Foundation

### Task 1: Add Skill ID Validation

**Files:**
- Modify: `src-tauri/src/skills.rs`
- Modify: `src-tauri/src/routes.rs`
- Test: `src-tauri/src/skills.rs`

**Steps:**
1. Extract shared validation for skill ids such as `validate_skill_id(id: &str) -> Result<()>`.
2. Allow only simple directory names, for example `a-z`, `0-9`, `.`, `_`, `-`, with no `/`, `\`, NUL, empty value, `.`, or `..`.
3. Use the validator in `scan_skills` route comparisons, `add_route`, `remove_route`, and `delete_skill`.
4. Add unit tests for valid ids, traversal attempts, empty ids, and dot paths.
5. Run `cargo test` in `src-tauri`.
6. Commit as `fix: validate skill ids`.

### Task 2: Make Path Containment Explicit

**Files:**
- Modify: `src-tauri/src/skills.rs`
- Modify: `src-tauri/src/routes.rs`

**Steps:**
1. Add helper logic that builds central/platform paths only after validation.
2. Canonicalize existing source directories before comparing symlink targets.
3. For missing platform targets, compare against a normalized intended source path.
4. Ensure `delete_skill` refuses to delete anything outside central.
5. Run `cargo test` and `cargo check`.
6. Commit as `fix: harden route path checks`.

### Task 3: Add Route Behavior Tests

**Files:**
- Modify: `src-tauri/src/routes.rs`
- Modify: `src-tauri/src/platforms.rs` if test hooks are needed

**Steps:**
1. Introduce testable helpers for symlink creation/removal that accept explicit central and platform directories.
2. Test idempotent add when the symlink already points to central.
3. Test conflict when target is a real directory.
4. Test conflict when target symlink points elsewhere.
5. Test remove refuses to delete real directories.
6. Run `cargo test`.
7. Commit as `test: cover route symlink behavior`.

## Phase 2: Real Skill Metadata

### Task 4: Parse SKILL.md Frontmatter Properly

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/skills.rs`
- Modify: `src/types.ts`

**Steps:**
1. Add a YAML/frontmatter parsing dependency, preferably a small Rust-side parser using `serde_yaml`.
2. Parse `name`, `description`, `version`, and `tags`.
3. Keep safe fallbacks when `SKILL.md` is absent or malformed.
4. Return `title`, `tagline`, `version`, and `tags` from real metadata.
5. Add parser unit tests for normal frontmatter, quoted descriptions, multiline body fallback, missing fields, and malformed YAML.
6. Run `cargo test`, `cargo check`, and `pnpm build`.
7. Commit as `feat: parse skill metadata`.

### Task 5: Add Skill Detail Command

**Files:**
- Modify: `src-tauri/src/skills.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/ipc.ts`
- Modify: `src/types.ts`
- Modify: `src/App.tsx`

**Steps:**
1. Add `SkillDetail` with `skill`, `readme` or `skillMd`, `sourcePath`, and `files`.
2. Implement `get_skill_detail(id)` with the same validation and containment checks.
3. Add IPC wrapper and TypeScript type.
4. Load detail when a skill is selected.
5. Replace the placeholder Files section with real file names, sizes, and modified times.
6. Run `cargo test`, `cargo check`, and `pnpm build`.
7. Commit as `feat: show skill details`.

## Phase 3: Frontend Usability

### Task 6: Replace Alert-Only Errors

**Files:**
- Modify: `src/App.tsx`

**Steps:**
1. Add a small app-level notice/toast state.
2. Replace `alert(...)` calls with non-blocking notices.
3. Show operation-specific error messages for scan, route toggle, import, delete, and config save.
4. Keep boot failure as a full-screen error.
5. Run `pnpm build`.
6. Commit as `feat: improve app error feedback`.

### Task 7: Add Pending States And Manual Refresh

**Files:**
- Modify: `src/App.tsx`

**Steps:**
1. Add pending state for `scan`, `toggleRoute`, `deleteSkill`, and `importSkill`.
2. Disable affected buttons while operations are running.
3. Add a Refresh button in the list header.
4. Preserve current selection after refresh when possible.
5. Run `pnpm build`.
6. Commit as `feat: add refresh and pending states`.

## Phase 4: Sync And Maintenance

### Task 8: Add File Watcher

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/watcher.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/App.tsx`

**Steps:**
1. Add `notify`.
2. Start a backend watcher for central skills and visible platform directories.
3. Debounce change events.
4. Emit `skills-changed`.
5. Listen in React and refresh skills.
6. Keep the manual Refresh button as a fallback.
7. Run `cargo check`, `cargo test`, and `pnpm build`.
8. Commit as `feat: refresh skills from file changes`.

### Task 9: Clean Up Documentation And Roadmap

**Files:**
- Modify: `README.md`
- Modify: `DEVELOPMENT.md`

**Steps:**
1. Update the phase checklist to reflect completed first-MVP work.
2. Move known issues into a short "Current Limitations" section.
3. Document local GitHub setup and basic dev commands.
4. Run `pnpm build` and `cargo check`.
5. Commit as `docs: update project roadmap`.

## Phase 5: AI Summary

### Task 10: Add Key Storage

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/ai.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/ipc.ts`
- Modify: `src/App.tsx`

**Steps:**
1. Add `keyring`.
2. Implement `get_api_key_status`, `set_api_key`, and `clear_api_key`.
3. Show API key settings without ever returning the secret to the UI.
4. Run `cargo check`, `cargo test`, and `pnpm build`.
5. Commit as `feat: store ai api key securely`.

### Task 11: Generate And Cache Summaries

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/ai.rs`
- Modify: `src-tauri/src/skills.rs`
- Modify: `src/App.tsx`

**Steps:**
1. Add HTTP client and SQLite cache dependencies.
2. Compute a content hash for `SKILL.md`.
3. Implement `generate_summary(skill_id, force)` with cache-first behavior.
4. Show summary loading, error, and regenerate states in the detail pane.
5. Fall back to frontmatter description when no key exists.
6. Run `cargo check`, `cargo test`, and `pnpm build`.
7. Commit as `feat: generate cached skill summaries`.

## Phase 6: Packaging

### Task 12: Prepare macOS Build

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `README.md`
- Create: `.github/workflows/release.yml` if release automation is desired

**Steps:**
1. Replace placeholder icon with a real app icon set.
2. Enable bundle config.
3. Verify `pnpm tauri build` locally.
4. Document unsigned local builds separately from signed release builds.
5. Add signing/notarization workflow only after Apple Developer credentials are ready.
6. Commit as `build: prepare macos packaging`.

## Recommended Immediate Order

1. Task 1
2. Task 2
3. Task 3
4. Task 4
5. Task 5

Stop before AI summary until safety, tests, and real metadata are stable.
