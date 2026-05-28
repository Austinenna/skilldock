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
