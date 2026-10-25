//! SkillDock core.
//!
//! Layout under the root:
//!   skills.toml      manifest: agents + which skill goes to which agent
//!   dev/<name>/      development sources, never loaded by agents
//!   released/<name>/ published snapshots; agent dirs only symlink here
//!   _sandbox/        project used by `test` to check auto-triggering
//!
//! Safety rule: we only create, fix or remove symlinks that point into
//! `<root>/released/`. Anything else found in an agent dir is reported, never touched.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::symlink;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use toml_edit::{Array, DocumentMut};

// ---------- errors ----------

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! bail {
    ($($t:tt)*) => { return Err(Error(format!($($t)*))) };
}

fn ctx<T>(r: io::Result<T>, what: impl fmt::Display) -> Result<T> {
    r.map_err(|e| Error(format!("{what}: {e}")))
}

// ---------- paths & root ----------

pub const MANIFEST: &str = "skills.toml";
pub const DEFAULT_EXCLUDE: &[&str] = &[".DS_Store", "__pycache__", ".git"];

pub fn home() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| Error("找不到 home 目录".into()))
}

pub fn expand_tilde(s: &str) -> Result<PathBuf> {
    if s == "~" {
        return home();
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return Ok(home()?.join(rest));
    }
    Ok(PathBuf::from(s))
}

/// Replace the home prefix with `~` for display and for writing config files.
pub fn tildify(p: &Path) -> String {
    if let Ok(h) = home() {
        if let Ok(rest) = p.strip_prefix(&h) {
            return format!("~/{}", rest.display());
        }
    }
    p.display().to_string()
}

pub fn config_file() -> Result<PathBuf> {
    Ok(home()?.join(".config/skilldock/config.toml"))
}

#[derive(Debug, Clone)]
pub enum RootSource {
    Env,
    Config(PathBuf),
    Default,
}

#[derive(Debug, Clone)]
pub struct Root {
    pub path: PathBuf,
    pub source: RootSource,
}

impl Root {
    pub fn dev(&self) -> PathBuf {
        self.path.join("dev")
    }
    pub fn released(&self) -> PathBuf {
        self.path.join("released")
    }
    pub fn sandbox(&self) -> PathBuf {
        self.path.join("_sandbox")
    }
    pub fn manifest_path(&self) -> PathBuf {
        self.path.join(MANIFEST)
    }
}

/// SKILLDOCK_HOME > ~/.config/skilldock/config.toml `root` > ~/.skilldock
pub fn resolve_root() -> Result<Root> {
    if let Ok(v) = std::env::var("SKILLDOCK_HOME") {
        if !v.is_empty() {
            return Ok(Root { path: expand_tilde(&v)?, source: RootSource::Env });
        }
    }
    let cfg = config_file()?;
    if let Ok(text) = fs::read_to_string(&cfg) {
        let doc: DocumentMut = text
            .parse()
            .map_err(|e| Error(format!("{} 解析失败: {e}", cfg.display())))?;
        if let Some(r) = doc.get("root").and_then(|i| i.as_str()) {
            return Ok(Root { path: expand_tilde(r)?, source: RootSource::Config(cfg) });
        }
    }
    Ok(Root { path: home()?.join(".skilldock"), source: RootSource::Default })
}

pub fn write_root_config(root: &Path) -> Result<PathBuf> {
    let cfg = config_file()?;
    let mut doc: DocumentMut = match fs::read_to_string(&cfg) {
        Ok(t) => t.parse().map_err(|e| Error(format!("{} 解析失败: {e}", cfg.display())))?,
        Err(_) => DocumentMut::new(),
    };
    doc["root"] = toml_edit::value(tildify(root));
    fs::create_dir_all(cfg.parent().unwrap())?;
    ctx(fs::write(&cfg, doc.to_string()), cfg.display())?;
    Ok(cfg)
}

/// Lexical normalisation (no filesystem access): resolves `.` and `..`.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn resolve_link(link: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        normalize(target)
    } else {
        normalize(&link.parent().unwrap_or(Path::new("/")).join(target))
    }
}

// ---------- names & frontmatter ----------

pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn check_name(name: &str) -> Result<()> {
    if !valid_name(name) {
        bail!("skill 名 `{name}` 不合法：只能用小写字母、数字和连字符，且以字母或数字开头");
    }
    Ok(())
}

#[derive(Debug, Default, Clone)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
}

pub fn read_frontmatter(skill_dir: &Path) -> Option<Frontmatter> {
    let text = fs::read_to_string(skill_dir.join("SKILL.md")).ok()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let rest = text.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&rest[..end]).ok()?;
    let get = |k: &str| yaml.get(k).and_then(|v| v.as_str()).map(|s| s.trim().to_string());
    Some(Frontmatter { name: get("name"), description: get("description") })
}

// ---------- manifest ----------

pub struct Manifest {
    path: PathBuf,
    doc: DocumentMut,
}

impl Manifest {
    pub fn load(root: &Root) -> Result<Self> {
        let path = root.manifest_path();
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                bail!("找不到清单 {}。先运行 `skl init`", path.display())
            }
            Err(e) => return Err(e.into()),
        };
        let doc = text
            .parse()
            .map_err(|e| Error(format!("{} 解析失败: {e}", path.display())))?;
        Ok(Manifest { path, doc })
    }

    /// Agents in file order, paths expanded.
    pub fn agents(&self) -> Result<Vec<(String, PathBuf)>> {
        let mut out = Vec::new();
        if let Some(t) = self.doc.get("agents").and_then(|i| i.as_table_like()) {
            for (k, v) in t.iter() {
                let Some(s) = v.as_str() else {
                    bail!("[agents] 中 `{k}` 的值必须是路径字符串");
                };
                out.push((k.to_string(), expand_tilde(s)?));
            }
        }
        Ok(out)
    }

    pub fn exclude(&self) -> Vec<String> {
        self.doc
            .get("publish")
            .and_then(|p| p.get("exclude"))
            .and_then(|e| e.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_else(|| DEFAULT_EXCLUDE.iter().map(|s| s.to_string()).collect())
    }

    /// `agent/entry` pairs that doctor should not report (agent-managed dirs such as `claude/synced`).
    pub fn doctor_ignore(&self) -> BTreeSet<String> {
        self.doc
            .get("doctor")
            .and_then(|p| p.get("ignore"))
            .and_then(|e| e.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    }

    pub fn skills(&self) -> Result<BTreeMap<String, Vec<String>>> {
        let mut out = BTreeMap::new();
        if let Some(t) = self.doc.get("skills").and_then(|i| i.as_table_like()) {
            for (k, v) in t.iter() {
                let Some(arr) = v.as_array() else {
                    bail!("[skills] 中 `{k}` 的值必须是 agent 名数组，例如 [\"claude\"]");
                };
                out.insert(
                    k.to_string(),
                    arr.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
                );
            }
        }
        Ok(out)
    }

    pub fn skill_agents(&self, name: &str) -> Result<Option<Vec<String>>> {
        Ok(self.skills()?.remove(name))
    }

    pub fn set_skill(&mut self, name: &str, agents: &[String]) {
        if self.doc.get("skills").and_then(|i| i.as_table_like()).is_none() {
            self.doc["skills"] = toml_edit::table();
        }
        let mut arr = Array::new();
        for a in agents {
            arr.push(a.as_str());
        }
        self.doc["skills"][name] = toml_edit::value(arr);
    }

    pub fn remove_skill(&mut self, name: &str) {
        if let Some(t) = self.doc.get_mut("skills").and_then(|i| i.as_table_like_mut()) {
            t.remove(name);
        }
    }

    pub fn save(&self) -> Result<()> {
        ctx(fs::write(&self.path, self.doc.to_string()), self.path.display())
    }

    /// Expand `all` to every configured agent, drop duplicates, validate names.
    pub fn expand_agents(&self, agents: &[String]) -> Result<Vec<String>> {
        let known: Vec<String> = self.agents()?.into_iter().map(|(k, _)| k).collect();
        let mut out: Vec<String> = Vec::new();
        for a in agents {
            let items = if a == "all" { known.clone() } else { vec![a.clone()] };
            for i in items {
                if !out.contains(&i) {
                    out.push(i);
                }
            }
        }
        self.check_agents(&out)?;
        Ok(out)
    }

    pub fn check_agents(&self, agents: &[String]) -> Result<()> {
        let known: BTreeSet<String> = self.agents()?.into_iter().map(|(k, _)| k).collect();
        for a in agents {
            if !known.contains(a) {
                bail!(
                    "未知 agent `{a}`。已配置：{}",
                    known.into_iter().collect::<Vec<_>>().join(", ")
                );
            }
        }
        Ok(())
    }
}

// ---------- tree copy & hash ----------

fn excluded(name: &std::ffi::OsStr, exclude: &[String]) -> bool {
    let n = name.to_string_lossy();
    exclude.iter().any(|e| e == n.as_ref())
}

pub fn copy_tree(src: &Path, dst: &Path, exclude: &[String]) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in ctx(fs::read_dir(src), src.display())? {
        let entry = entry?;
        let name = entry.file_name();
        if excluded(&name, exclude) {
            continue;
        }
        let s = entry.path();
        let d = dst.join(&name);
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            symlink(fs::read_link(&s)?, &d)?;
        } else if ft.is_dir() {
            copy_tree(&s, &d, exclude)?;
        } else {
            ctx(fs::copy(&s, &d), s.display())?;
            // Keep mtime so dev/released can be compared by metadata (see `same_tree`).
            if let Ok(m) = fs::metadata(&s).and_then(|m| m.modified()) {
                let _ = fs::File::open(&d).and_then(|f| f.set_modified(m));
            }
        }
    }
    Ok(())
}

/// Content hash of a tree, ignoring excluded names. Used to compare dev vs released.
pub fn tree_hash(dir: &Path, exclude: &[String]) -> Result<String> {
    fn walk(base: &Path, dir: &Path, exclude: &[String], h: &mut Sha256) -> Result<()> {
        let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<io::Result<_>>()?;
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            if excluded(&e.file_name(), exclude) {
                continue;
            }
            let p = e.path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
            let ft = e.file_type()?;
            h.update(rel.as_bytes());
            h.update([0]);
            if ft.is_symlink() {
                h.update(b"L");
                h.update(fs::read_link(&p)?.to_string_lossy().as_bytes());
            } else if ft.is_dir() {
                h.update(b"D");
                walk(base, &p, exclude, h)?;
            } else {
                h.update(b"F");
                h.update(fs::read(&p)?);
            }
            h.update([0]);
        }
        Ok(())
    }
    let mut h = Sha256::new();
    walk(dir, dir, exclude, &mut h)?;
    Ok(format!("{:x}", h.finalize()))
}

type Sig = Vec<(String, u8, u64, Option<std::time::SystemTime>)>;

/// Cheap signature: relative path, kind, size, mtime of every entry.
fn tree_sig(dir: &Path, exclude: &[String]) -> Result<Sig> {
    fn walk(base: &Path, dir: &Path, exclude: &[String], out: &mut Sig) -> Result<()> {
        for e in fs::read_dir(dir)? {
            let e = e?;
            if excluded(&e.file_name(), exclude) {
                continue;
            }
            let p = e.path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
            let ft = e.file_type()?;
            if ft.is_dir() {
                out.push((rel, b'D', 0, None));
                walk(base, &p, exclude, out)?;
            } else {
                let m = fs::symlink_metadata(&p)?;
                let kind = if ft.is_symlink() { b'L' } else { b'F' };
                let mtime = if ft.is_symlink() { None } else { m.modified().ok() };
                out.push((rel, kind, m.len(), mtime));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, exclude, &mut out)?;
    out.sort();
    Ok(out)
}

/// True when `released` holds the same content as `dev`.
/// Fast path compares metadata; on mismatch falls back to content hashing and,
/// if contents turn out equal, copies dev mtimes onto released so the next check is fast.
pub fn same_tree(dev: &Path, released: &Path, exclude: &[String]) -> Result<bool> {
    let a = tree_sig(dev, exclude)?;
    let b = tree_sig(released, exclude)?;
    if a == b {
        return Ok(true);
    }
    let same_shape = a.len() == b.len()
        && a.iter().zip(&b).all(|(x, y)| x.0 == y.0 && x.1 == y.1 && x.2 == y.2);
    if !same_shape || tree_hash(dev, exclude)? != tree_hash(released, exclude)? {
        return Ok(false);
    }
    for (rel, kind, _, mtime) in &a {
        if let (b'F', Some(m)) = (*kind, mtime) {
            let _ = fs::File::open(released.join(rel)).and_then(|f| f.set_modified(*m));
        }
    }
    Ok(true)
}

// ---------- diff ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    /// Path relative to the skill dir.
    pub path: String,
    pub kind: ChangeKind,
    /// Size in released/ (0 when added).
    pub old_size: u64,
    /// Size in dev/ (0 when removed).
    pub new_size: u64,
    /// Neither side could be shown as text.
    pub binary: bool,
    /// Unified diff for modified text files, when requested.
    pub patch: Option<String>,
}

/// Files above this size are listed but not diffed line by line.
const DIFF_MAX_BYTES: u64 = 1 << 20;

enum Leaf {
    File(PathBuf, u64, Option<std::time::SystemTime>),
    Link(PathBuf),
}

/// Every file and symlink under `dir` (dirs themselves are implied by their contents).
fn leaves(dir: &Path, exclude: &[String]) -> Result<BTreeMap<String, Leaf>> {
    fn walk(base: &Path, dir: &Path, exclude: &[String], out: &mut BTreeMap<String, Leaf>) -> Result<()> {
        for e in fs::read_dir(dir)? {
            let e = e?;
            if excluded(&e.file_name(), exclude) {
                continue;
            }
            let p = e.path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
            let ft = e.file_type()?;
            if ft.is_symlink() {
                out.insert(rel, Leaf::Link(fs::read_link(&p)?));
            } else if ft.is_dir() {
                walk(base, &p, exclude, out)?;
            } else {
                let m = fs::metadata(&p)?;
                out.insert(rel, Leaf::File(p, m.len(), m.modified().ok()));
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    if dir.is_dir() {
        walk(dir, dir, exclude, &mut out)?;
    }
    Ok(out)
}

fn as_text(bytes: &[u8]) -> Option<&str> {
    if bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok()
}

fn leaf_size(l: &Leaf) -> u64 {
    match l {
        Leaf::File(_, n, _) => *n,
        Leaf::Link(t) => t.as_os_str().len() as u64,
    }
}

fn leaf_is_text(l: &Leaf) -> bool {
    match l {
        Leaf::Link(_) => true,
        Leaf::File(p, n, _) => *n > DIFF_MAX_BYTES || fs::read(p).map(|b| as_text(&b).is_some()).unwrap_or(false),
    }
}

/// What publishing `dev/<name>` would change in `released/<name>`.
/// A skill that was never released shows every file as added.
/// Uses the same exclude list as publish, so `skl ls` and `skl diff` agree.
pub fn diff_skill(root: &Root, name: &str, with_patch: bool) -> Result<Vec<FileChange>> {
    check_name(name)?;
    let dev = root.dev().join(name);
    let rel = root.released().join(name);
    if !is_real_dir(&dev) {
        if !is_real_dir(&rel) {
            bail!("没有 `{name}` 这个 skill");
        }
        bail!("`{name}` 没有开发区（dev/{name}），没有可比较的改动");
    }
    let exclude = Manifest::load(root)?.exclude();
    let new = leaves(&dev, &exclude)?;
    let old = leaves(&rel, &exclude)?;
    let mut out = Vec::new();
    for (path, n) in &new {
        let Some(o) = old.get(path) else {
            out.push(FileChange {
                path: path.clone(),
                kind: ChangeKind::Added,
                old_size: 0,
                new_size: leaf_size(n),
                binary: !leaf_is_text(n),
                patch: None,
            });
            continue;
        };
        let (old_text, new_text, binary) = match (o, n) {
            (Leaf::Link(a), Leaf::Link(b)) => {
                if a == b {
                    continue;
                }
                (format!("-> {}\n", a.display()), format!("-> {}\n", b.display()), false)
            }
            (Leaf::File(op, os, om), Leaf::File(np, ns, nm)) => {
                if os == ns && om.is_some() && om == nm {
                    continue;
                }
                let a = fs::read(op)?;
                let b = fs::read(np)?;
                if a == b {
                    continue;
                }
                match (as_text(&a), as_text(&b)) {
                    (Some(x), Some(y)) if *os <= DIFF_MAX_BYTES && *ns <= DIFF_MAX_BYTES => {
                        (x.to_string(), y.to_string(), false)
                    }
                    (Some(_), Some(_)) => (String::new(), String::new(), false),
                    _ => (String::new(), String::new(), true),
                }
            }
            // File replaced by symlink or the other way round.
            _ => (String::new(), String::new(), false),
        };
        let patch = (with_patch && !binary && (!old_text.is_empty() || !new_text.is_empty())).then(|| {
            similar::TextDiff::from_lines(&old_text, &new_text)
                .unified_diff()
                .context_radius(3)
                .to_string()
        });
        out.push(FileChange {
            path: path.clone(),
            kind: ChangeKind::Modified,
            old_size: leaf_size(o),
            new_size: leaf_size(n),
            binary,
            patch,
        });
    }
    for (path, o) in &old {
        if !new.contains_key(path) {
            out.push(FileChange {
                path: path.clone(),
                kind: ChangeKind::Removed,
                old_size: leaf_size(o),
                new_size: 0,
                binary: !leaf_is_text(o),
                patch: None,
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn is_real_dir(p: &Path) -> bool {
    fs::symlink_metadata(p).map(|m| m.is_dir()).unwrap_or(false)
}

/// Copy `src` into `released/<name>` atomically (staging dir + rename).
fn snapshot(root: &Root, src: &Path, name: &str, exclude: &[String]) -> Result<()> {
    let released = root.released();
    fs::create_dir_all(&released)?;
    let staging = released.join(format!(".staging-{name}"));
    let old = released.join(format!(".old-{name}"));
    let target = released.join(name);
    for p in [&staging, &old] {
        if fs::symlink_metadata(p).is_ok() {
            fs::remove_dir_all(p)?;
        }
    }
    copy_tree(src, &staging, exclude)?;
    if let Ok(m) = fs::symlink_metadata(&target) {
        if !m.is_dir() {
            fs::remove_dir_all(&staging).ok();
            bail!("{} 不是真实目录，拒绝覆盖", target.display());
        }
        fs::rename(&target, &old)?;
    }
    if let Err(e) = fs::rename(&staging, &target) {
        if old.exists() {
            fs::rename(&old, &target).ok();
        }
        return Err(e.into());
    }
    if old.exists() {
        fs::remove_dir_all(&old)?;
    }
    Ok(())
}

// ---------- git ----------

fn git_commit(root: &Root, msg: &str, paths: &[&str]) -> Option<String> {
    if !root.path.join(".git").exists() {
        return None;
    }
    let run = |args: &[&str]| {
        Command::new("git").arg("-C").arg(&root.path).args(args).output()
    };
    let mut add = vec!["add", "-A", "--"];
    add.extend(paths);
    if !run(&add).map(|o| o.status.success()).unwrap_or(false) {
        return Some("git add 失败，未提交".into());
    }
    let mut diff = vec!["diff", "--cached", "--quiet", "--"];
    diff.extend(paths);
    if run(&diff).map(|o| o.status.success()).unwrap_or(false) {
        return Some("内容无变化，未提交".into());
    }
    let mut commit = vec!["commit", "-q", "-m", msg, "--"];
    commit.extend(paths);
    match run(&commit) {
        Ok(o) if o.status.success() => Some(format!("已提交：{msg}")),
        Ok(o) => Some(format!("git commit 失败：{}", String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) => Some(format!("git commit 失败：{e}")),
    }
}

// ---------- init / new ----------

const GITIGNORE: &str = "_sandbox/\n.trash/\nnode_modules/\n__pycache__/\n.DS_Store\n.staging-*\n.old-*\n";

fn manifest_template() -> Result<String> {
    let mut s = String::from("# SkillDock 清单：见 https://github.com/Austinenna/skilldock/blob/main/docs/redesign.md\n\n[agents]\n");
    let h = home()?;
    for (id, dir) in [
        ("claude", ".claude/skills"),
        ("codex", ".codex/skills"),
        ("workbuddy", ".workbuddy/skills"),
        ("openclaw", ".openclaw/skills"),
        ("kairos", ".kairos/skills"),
    ] {
        let parent = h.join(dir).parent().unwrap().to_path_buf();
        let line = format!("{id} = \"~/{dir}\"\n");
        if parent.exists() {
            s.push_str(&line);
        } else {
            s.push_str(&format!("# {line}"));
        }
    }
    s.push_str("\n[publish]\nexclude = [");
    s.push_str(&DEFAULT_EXCLUDE.iter().map(|e| format!("\"{e}\"")).collect::<Vec<_>>().join(", "));
    s.push_str("]\n\n[doctor]\n# agent 自己维护的条目，doctor 不报告（格式 agent/条目名）\nignore = [\"claude/synced\"]\n\n[skills]\n");
    Ok(s)
}

pub fn init(root: &Root) -> Result<Vec<String>> {
    let mut done = Vec::new();
    for d in [root.dev(), root.released()] {
        if !d.exists() {
            fs::create_dir_all(&d)?;
            done.push(format!("创建 {}", tildify(&d)));
        }
    }
    let m = root.manifest_path();
    if !m.exists() {
        fs::write(&m, manifest_template()?)?;
        done.push(format!("创建 {}", tildify(&m)));
    }
    let gi = root.path.join(".gitignore");
    if !gi.exists() {
        fs::write(&gi, GITIGNORE)?;
        done.push(format!("创建 {}", tildify(&gi)));
    }
    if !root.path.join(".git").exists() {
        let ok = Command::new("git")
            .arg("-C")
            .arg(&root.path)
            .args(["init", "-q"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            done.push("git init".into());
        }
    }
    Ok(done)
}

pub fn new_skill(root: &Root, name: &str) -> Result<PathBuf> {
    check_name(name)?;
    let dir = root.dev().join(name);
    if fs::symlink_metadata(&dir).is_ok() {
        bail!("{} 已存在", tildify(&dir));
    }
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join("SKILL.md"),
        format!(
            "---\nname: {name}\ndescription: TODO 一句话写清楚做什么、在什么情况下应该使用。\n---\n\n# {name}\n\n"
        ),
    )?;
    Ok(dir)
}

// ---------- status ----------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    DevOnly,
    Released,
    DevAhead,
    ReleasedOnly,
    ManifestOnly,
}

impl State {
    pub fn label(&self) -> &'static str {
        match self {
            State::DevOnly => "开发中",
            State::Released => "已发布",
            State::DevAhead => "有未发布改动",
            State::ReleasedOnly => "已发布（无开发源）",
            State::ManifestOnly => "清单里有但未发布",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SkillStatus {
    pub name: String,
    pub state: State,
    pub agents: Vec<String>,
    pub description: Option<String>,
}

fn list_dirs(dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if !n.starts_with('.') && is_real_dir(&e.path()) {
                out.insert(n);
            }
        }
    }
    out
}

pub fn status(root: &Root) -> Result<Vec<SkillStatus>> {
    let manifest = Manifest::load(root)?;
    let skills = manifest.skills()?;
    let exclude = manifest.exclude();
    let dev = list_dirs(&root.dev());
    let rel = list_dirs(&root.released());
    let mut names: BTreeSet<String> = dev.union(&rel).cloned().collect();
    names.extend(skills.keys().cloned());
    let mut out = Vec::new();
    for name in names {
        let d = dev.contains(&name);
        let r = rel.contains(&name);
        let state = match (d, r) {
            (true, false) => State::DevOnly,
            (false, true) => State::ReleasedOnly,
            (false, false) => State::ManifestOnly,
            (true, true) => {
                if same_tree(&root.dev().join(&name), &root.released().join(&name), &exclude)? {
                    State::Released
                } else {
                    State::DevAhead
                }
            }
        };
        let src = if d { root.dev().join(&name) } else { root.released().join(&name) };
        out.push(SkillStatus {
            description: read_frontmatter(&src).and_then(|f| f.description),
            agents: skills.get(&name).cloned().unwrap_or_default(),
            name,
            state,
        });
    }
    Ok(out)
}

// ---------- sync ----------

#[derive(Debug, Clone)]
pub enum Action {
    Created,
    Kept,
    Fixed(PathBuf),
    Removed,
    Conflict(String),
}

#[derive(Debug, Clone)]
pub struct SyncItem {
    pub agent: String,
    pub name: String,
    pub path: PathBuf,
    pub action: Action,
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub items: Vec<SyncItem>,
    pub notes: Vec<String>,
}

fn released_prefixes(root: &Root) -> Vec<PathBuf> {
    let mut v = vec![normalize(&root.released())];
    if let Ok(c) = fs::canonicalize(&root.path) {
        let c = c.join("released");
        if !v.contains(&c) {
            v.push(c);
        }
    }
    v
}

fn managed_target(root: &Root, link: &Path) -> Option<PathBuf> {
    let t = fs::read_link(link).ok()?;
    let abs = resolve_link(link, &t);
    released_prefixes(root)
        .iter()
        .any(|p| abs.starts_with(p) && abs != *p)
        .then_some(abs)
}

fn points_to(root: &Root, link: &Path, name: &str) -> bool {
    match managed_target(root, link) {
        Some(abs) => released_prefixes(root).iter().any(|p| abs == p.join(name)),
        None => false,
    }
}

pub fn sync(root: &Root, dry_run: bool) -> Result<SyncReport> {
    let manifest = Manifest::load(root)?;
    let skills = manifest.skills()?;
    let released = root.released();
    let mut rep = SyncReport::default();

    for (name, agents) in &skills {
        if !is_real_dir(&released.join(name)) {
            rep.notes.push(format!("清单里的 `{name}` 没有发布版（released/{name}），跳过"));
        }
        if let Err(e) = manifest.check_agents(agents) {
            rep.notes.push(format!("`{name}`: {e}"));
        }
    }

    for (agent, dir) in manifest.agents()? {
        if !dir.exists() {
            let parent_ok = dir.parent().map(|p| p.exists()).unwrap_or(false);
            if !parent_ok {
                rep.notes.push(format!("{agent}: {} 的上级目录不存在（未安装？），跳过", tildify(&dir)));
                continue;
            }
            if !dry_run {
                fs::create_dir_all(&dir)?;
            }
        }
        let desired: BTreeSet<&String> = skills
            .iter()
            .filter(|(n, a)| a.contains(&agent) && is_real_dir(&released.join(n)))
            .map(|(n, _)| n)
            .collect();

        for name in &desired {
            let p = dir.join(name);
            let target = released.join(name);
            let action = match fs::symlink_metadata(&p) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    if !dry_run {
                        ctx(symlink(&target, &p), p.display())?;
                    }
                    Action::Created
                }
                Err(e) => return Err(e.into()),
                Ok(m) if m.file_type().is_symlink() => {
                    if points_to(root, &p, name) {
                        Action::Kept
                    } else if let Some(old) = managed_target(root, &p) {
                        if !dry_run {
                            fs::remove_file(&p)?;
                            symlink(&target, &p)?;
                        }
                        Action::Fixed(old)
                    } else {
                        let t = fs::read_link(&p).unwrap_or_default();
                        Action::Conflict(format!("已有指向其他位置的软链接 → {}", t.display()))
                    }
                }
                Ok(m) => Action::Conflict(if m.is_dir() {
                    "已有同名真实目录".into()
                } else {
                    "已有同名文件".into()
                }),
            };
            rep.items.push(SyncItem { agent: agent.clone(), name: name.to_string(), path: p, action });
        }

        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with('.') || desired.contains(&n) {
                    continue;
                }
                let p = e.path();
                let is_link = fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(false);
                if is_link && managed_target(root, &p).is_some() {
                    if !dry_run {
                        fs::remove_file(&p)?;
                    }
                    rep.items.push(SyncItem { agent: agent.clone(), name: n, path: p, action: Action::Removed });
                }
            }
        }
    }
    Ok(rep)
}

// ---------- publish / adopt / unpublish ----------

pub struct PublishReport {
    pub agents: Vec<String>,
    pub sync: SyncReport,
    pub git: Option<String>,
    pub warnings: Vec<String>,
}

fn check_skill_dir(dir: &Path, name: &str, strict: bool) -> Result<Vec<String>> {
    let mut warns = Vec::new();
    match read_frontmatter(dir) {
        None => {
            if !dir.join("SKILL.md").exists() {
                bail!("{} 缺少 SKILL.md", tildify(dir));
            }
            let msg = format!("{} 的 SKILL.md frontmatter 无法解析", tildify(dir));
            if strict {
                bail!("{msg}");
            }
            warns.push(msg);
        }
        Some(fm) => {
            if fm.name.as_deref() != Some(name) {
                let msg = format!(
                    "SKILL.md 的 name 是 `{}`，与目录名 `{name}` 不一致",
                    fm.name.unwrap_or_default()
                );
                if strict {
                    bail!("{msg}");
                }
                warns.push(msg);
            }
            if fm.description.as_deref().map(str::is_empty).unwrap_or(true) {
                warns.push("SKILL.md 缺少 description，agent 无法判断何时触发".into());
            }
        }
    }
    Ok(warns)
}

fn finish(root: &Root, manifest: &mut Manifest, name: &str, to: Option<Vec<String>>, msg: &str, warnings: Vec<String>) -> Result<PublishReport> {
    let mut warnings = warnings;
    // No --to: an update keeps its agents; a first publish goes to every agent.
    let agents = match to {
        Some(a) => manifest.expand_agents(&a)?,
        None => match manifest.skill_agents(name)? {
            Some(a) => a,
            None => {
                let all = manifest.expand_agents(&["all".to_string()])?;
                warnings.push(format!("首次发布未指定 --to，默认分发给全部 agent：{}", all.join(", ")));
                all
            }
        },
    };
    manifest.set_skill(name, &agents);
    manifest.save()?;
    let sync = sync(root, false)?;
    let dev_rel = format!("dev/{name}");
    let rel_rel = format!("released/{name}");
    let git = git_commit(root, msg, &[&dev_rel, &rel_rel, MANIFEST]);
    Ok(PublishReport { agents, sync, git, warnings })
}

fn parse_agents_check(manifest: &Manifest, to: &Option<Vec<String>>) -> Result<()> {
    if let Some(a) = to {
        manifest.expand_agents(a)?;
    }
    Ok(())
}

pub fn publish(root: &Root, name: &str, to: Option<Vec<String>>) -> Result<PublishReport> {
    check_name(name)?;
    let src = root.dev().join(name);
    if !is_real_dir(&src) {
        bail!("开发区没有 `{name}`（{}）", tildify(&src));
    }
    let warnings = check_skill_dir(&src, name, true)?;
    let mut manifest = Manifest::load(root)?;
    parse_agents_check(&manifest, &to)?;
    snapshot(root, &src, name, &manifest.exclude())?;
    finish(root, &mut manifest, name, to, &format!("publish: {name}"), warnings)
}

pub struct AdoptOptions {
    pub name: Option<String>,
    pub to: Option<Vec<String>>,
    pub remove_source: bool,
}

/// Take an existing skill directory (e.g. a copy living in an agent dir) as the released version.
pub fn adopt(root: &Root, src: &Path, opts: AdoptOptions) -> Result<(String, PublishReport)> {
    let meta = ctx(fs::symlink_metadata(src), src.display())?;
    if meta.file_type().is_symlink() {
        bail!("{} 是软链接，请直接传它指向的真实目录", src.display());
    }
    if !meta.is_dir() {
        bail!("{} 不是目录", src.display());
    }
    let name = match opts.name {
        Some(n) => n,
        None => src.file_name().unwrap().to_string_lossy().into_owned(),
    };
    check_name(&name)?;
    let target = root.released().join(&name);
    if fs::symlink_metadata(&target).is_ok() {
        bail!("released/{name} 已存在；更新请改 dev/{name} 后 skl publish");
    }
    let mut warnings = check_skill_dir(src, &name, false)?;
    let mut manifest = Manifest::load(root)?;
    parse_agents_check(&manifest, &opts.to)?;
    let exclude = manifest.exclude();
    snapshot(root, src, &name, &exclude)?;

    let dev = root.dev().join(&name);
    if fs::symlink_metadata(&dev).is_err() {
        copy_tree(src, &dev, &[".DS_Store".to_string()])?;
        warnings.push(format!("开发区没有同名 skill，已复制一份到 dev/{name}"));
    }

    if opts.remove_source {
        let a = tree_hash(src, &exclude)?;
        let b = tree_hash(&target, &exclude)?;
        if a != b {
            bail!("校验失败：released/{name} 与源目录内容不一致，未删除源目录");
        }
        fs::remove_dir_all(src)?;
    }
    let rep = finish(root, &mut manifest, &name, opts.to, &format!("adopt: {name}"), warnings)?;
    Ok((name, rep))
}

pub fn unpublish(root: &Root, name: &str) -> Result<SyncReport> {
    check_name(name)?;
    let mut manifest = Manifest::load(root)?;
    manifest.remove_skill(name);
    manifest.save()?;
    let target = root.released().join(name);
    if is_real_dir(&target) {
        fs::remove_dir_all(&target)?;
    }
    let rep = sync(root, false)?;
    let rel_rel = format!("released/{name}");
    git_commit(root, &format!("unpublish: {name}"), &[&rel_rel, MANIFEST]);
    Ok(rep)
}

/// Change which agents an already released skill is distributed to.
pub fn set_agents(root: &Root, name: &str, agents: &[String]) -> Result<SyncReport> {
    check_name(name)?;
    if !is_real_dir(&root.released().join(name)) {
        bail!("`{name}` 尚未发布，先 skl publish");
    }
    let mut manifest = Manifest::load(root)?;
    let agents = &manifest.expand_agents(agents)?;
    manifest.set_skill(name, agents);
    manifest.save()?;
    let rep = sync(root, false)?;
    git_commit(root, &format!("route: {name} → [{}]", agents.join(", ")), &[MANIFEST]);
    Ok(rep)
}

/// Add agents to a released skill without republishing it.
pub fn link(root: &Root, name: &str, add: &[String]) -> Result<(Vec<String>, SyncReport)> {
    let manifest = Manifest::load(root)?;
    let mut agents = manifest.skill_agents(name)?.unwrap_or_default();
    for a in manifest.expand_agents(add)? {
        if !agents.contains(&a) {
            agents.push(a);
        }
    }
    let rep = set_agents(root, name, &agents)?;
    Ok((agents, rep))
}

/// Remove agents from a released skill; the release itself stays.
pub fn unlink(root: &Root, name: &str, remove: &[String]) -> Result<(Vec<String>, SyncReport)> {
    let manifest = Manifest::load(root)?;
    let remove = manifest.expand_agents(remove)?;
    let agents: Vec<String> = manifest
        .skill_agents(name)?
        .unwrap_or_default()
        .into_iter()
        .filter(|a| !remove.contains(a))
        .collect();
    let rep = set_agents(root, name, &agents)?;
    Ok((agents, rep))
}

pub struct DeleteReport {
    pub trashed: Option<PathBuf>,
    pub unpublished: Option<SyncReport>,
}

/// Delete a skill entirely: unpublish it if released, then move `dev/<name>`
/// into `<root>/.trash/` (not removed, so a mistaken delete can be undone).
pub fn delete(root: &Root, name: &str) -> Result<DeleteReport> {
    check_name(name)?;
    let dev = root.dev().join(name);
    let released = is_real_dir(&root.released().join(name));
    let in_manifest = Manifest::load(root)?.skill_agents(name)?.is_some();
    if !is_real_dir(&dev) && !released && !in_manifest {
        bail!("没有 `{name}` 这个 skill");
    }
    let unpublished = if released || in_manifest { Some(unpublish(root, name)?) } else { None };
    let trashed = if is_real_dir(&dev) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let trash = root.path.join(".trash");
        fs::create_dir_all(&trash)?;
        let dst = trash.join(format!("{name}-{stamp}"));
        fs::rename(&dev, &dst)?;
        let dev_rel = format!("dev/{name}");
        git_commit(root, &format!("delete: {name}"), &[&dev_rel]);
        Some(dst)
    } else {
        None
    };
    Ok(DeleteReport { trashed, unpublished })
}

pub fn skill_path(root: &Root, name: &str) -> Result<PathBuf> {
    check_name(name)?;
    for p in [root.dev().join(name), root.released().join(name)] {
        if is_real_dir(&p) {
            return Ok(p);
        }
    }
    bail!("没有 `{name}` 这个 skill")
}

/// Top-level entries of a skill dir, for display.
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
}

pub fn list_entries(dir: &Path) -> Vec<Entry> {
    let mut out: Vec<Entry> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name() != ".DS_Store")
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some(Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: m.is_dir(),
                size: if m.is_file() { m.len() } else { 0 },
                modified: m.modified().ok(),
            })
        })
        .collect();
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    out
}

// ---------- test sandbox ----------

pub const SANDBOX_DIRS: &[&str] = &[".claude/skills", ".agents/skills"];

pub fn test_link(root: &Root, name: &str, clean: bool) -> Result<PathBuf> {
    check_name(name)?;
    let src = root.dev().join(name);
    if !clean && !is_real_dir(&src) {
        bail!("开发区没有 `{name}`");
    }
    let sb = root.sandbox();
    for sub in SANDBOX_DIRS {
        let d = sb.join(sub);
        let p = d.join(name);
        let is_link = fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).ok();
        if clean {
            if is_link == Some(true) {
                fs::remove_file(&p)?;
            }
            continue;
        }
        fs::create_dir_all(&d)?;
        match is_link {
            Some(true) => fs::remove_file(&p)?,
            Some(false) => bail!("{} 已存在且不是软链接", p.display()),
            None => {}
        }
        symlink(&src, &p)?;
    }
    Ok(sb)
}

// ---------- doctor ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub level: Level,
    pub msg: String,
}

pub fn doctor(root: &Root) -> Result<Vec<Finding>> {
    let mut out = Vec::new();
    let mut push = |level, msg: String| out.push(Finding { level, msg });
    let manifest = Manifest::load(root)?;
    let skills = manifest.skills()?;
    let agents = manifest.agents()?;
    let exclude = manifest.exclude();
    let ignore = manifest.doctor_ignore();
    let dev = list_dirs(&root.dev());
    let rel = list_dirs(&root.released());

    for (name, a) in &skills {
        if !rel.contains(name) {
            push(Level::Error, format!("清单里的 `{name}` 没有发布版 released/{name}"));
        }
        if let Err(e) = manifest.check_agents(a) {
            push(Level::Error, format!("`{name}`: {e}"));
        }
    }
    for n in &rel {
        if !skills.contains_key(n) {
            push(Level::Warn, format!("released/{n} 不在清单里（不会分发）"));
        }
    }
    if let Ok(rd) = fs::read_dir(root.released()) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with(".staging-") || n.starts_with(".old-") {
                push(Level::Warn, format!("released/{n} 是中断的发布残留，可删除"));
            }
        }
    }
    for (area, set, base) in [("dev", &dev, root.dev()), ("released", &rel, root.released())] {
        for n in set {
            let d = base.join(n);
            if !valid_name(n) {
                push(Level::Error, format!("{area}/{n}: 目录名不合法（只能小写字母、数字、连字符）"));
            }
            // git does not carry node_modules, so after a fresh clone deps are missing.
            if d.join("package.json").exists() && !d.join("node_modules").exists() {
                let fix = if area == "dev" {
                    format!("cd dev/{n} && npm ci")
                } else {
                    format!("先在 dev/{n} 里 npm ci，再 skl publish {n}")
                };
                push(Level::Warn, format!("{area}/{n}: 有 package.json 但未安装依赖（{fix}）"));
            }
            match read_frontmatter(&d) {
                None if !d.join("SKILL.md").exists() => push(Level::Error, format!("{area}/{n}: 缺少 SKILL.md")),
                None => push(Level::Error, format!("{area}/{n}: SKILL.md frontmatter 无法解析")),
                Some(fm) => {
                    if fm.name.as_deref() != Some(n.as_str()) {
                        push(Level::Warn, format!("{area}/{n}: SKILL.md name 为 `{}`，与目录名不一致", fm.name.unwrap_or_default()));
                    }
                    if fm.description.as_deref().map(str::is_empty).unwrap_or(true) {
                        push(Level::Warn, format!("{area}/{n}: 缺少 description"));
                    }
                }
            }
        }
    }
    for n in dev.intersection(&rel) {
        if !same_tree(&root.dev().join(n), &root.released().join(n), &exclude)? {
            push(Level::Info, format!("`{n}`: 开发版有未发布改动"));
        }
    }
    for n in rel.difference(&dev) {
        push(Level::Info, format!("`{n}`: 只有发布版，没有开发源"));
    }

    for (agent, dir) in &agents {
        if !dir.exists() {
            push(Level::Info, format!("{agent}: {} 不存在", tildify(dir)));
            continue;
        }
        let want: BTreeSet<&String> = skills.iter().filter(|(_, a)| a.contains(agent)).map(|(n, _)| n).collect();
        for n in &want {
            let p = dir.join(n);
            match fs::symlink_metadata(&p) {
                Err(_) => push(Level::Warn, format!("{agent}/{n}: 缺少链接（运行 skl sync）")),
                Ok(m) if m.file_type().is_symlink() => {
                    if !points_to(root, &p, n) {
                        push(Level::Warn, format!("{agent}/{n}: 链接指向不对（运行 skl sync）"));
                    } else if !p.exists() {
                        push(Level::Error, format!("{agent}/{n}: 链接失效"));
                    }
                }
                Ok(_) => push(Level::Error, format!("{agent}/{n}: 同名真实目录挡住了链接，需手动处理")),
            }
        }
        let Ok(rd) = fs::read_dir(dir) else { continue };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with('.') || want.contains(&n) || ignore.contains(&format!("{agent}/{n}")) {
                continue;
            }
            let p = e.path();
            let Ok(m) = fs::symlink_metadata(&p) else { continue };
            if m.file_type().is_symlink() {
                if managed_target(root, &p).is_some() {
                    push(Level::Warn, format!("{agent}/{n}: 多余的 skl 链接（运行 skl sync 清理）"));
                } else if !p.exists() {
                    push(Level::Warn, format!("{agent}/{n}: 断链 → {}", fs::read_link(&p).unwrap_or_default().display()));
                } else {
                    push(Level::Info, format!("{agent}/{n}: 指向外部的链接，未纳管"));
                }
            } else if m.is_dir() {
                push(Level::Info, format!("{agent}/{n}: 未纳管的真实目录（可用 skl adopt 收编）"));
            } else {
                push(Level::Info, format!("{agent}/{n}: 未纳管的文件"));
            }
        }
    }
    out.sort_by_key(|f| f.level);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid_name("wechat-gzh"));
        assert!(valid_name("1password"));
        assert!(!valid_name("Code"));
        assert!(!valid_name("-x"));
        assert!(!valid_name("a b"));
        assert!(!valid_name("../x"));
        assert!(!valid_name(""));
    }

    #[test]
    fn normalize_paths() {
        assert_eq!(normalize(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
        assert_eq!(
            resolve_link(Path::new("/x/agent/skills/foo"), Path::new("../../root/released/foo")),
            PathBuf::from("/x/root/released/foo")
        );
    }

    #[test]
    fn diff_lists_changes() {
        let dir = std::env::temp_dir().join(format!("skl-diff-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let root = Root { path: dir.clone(), source: RootSource::Default };
        let (dev, rel) = (root.dev().join("s"), root.released().join("s"));
        for d in [&dev, &rel] {
            fs::create_dir_all(d).unwrap();
        }
        fs::write(root.manifest_path(), "[agents]\n").unwrap();
        fs::write(rel.join("SKILL.md"), "a\nb\n").unwrap();
        fs::write(dev.join("SKILL.md"), "a\nc\n").unwrap();
        fs::write(rel.join("old.txt"), "x").unwrap();
        fs::write(rel.join("same.txt"), "y").unwrap();
        fs::write(dev.join("same.txt"), "y").unwrap();
        fs::write(dev.join("img.bin"), [0u8, 1, 2]).unwrap();
        fs::write(dev.join(".DS_Store"), "junk").unwrap();

        let got: Vec<_> = diff_skill(&root, "s", true).unwrap();
        let summary: Vec<_> = got.iter().map(|c| (c.path.as_str(), c.kind, c.binary)).collect();
        assert_eq!(
            summary,
            [
                ("SKILL.md", ChangeKind::Modified, false),
                ("img.bin", ChangeKind::Added, true),
                ("old.txt", ChangeKind::Removed, false),
            ]
        );
        let patch = got[0].patch.as_deref().unwrap();
        assert!(patch.contains("-b\n") && patch.contains("+c\n"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
