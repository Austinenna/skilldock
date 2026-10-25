use skilldock_core::*;
use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "skl —— SkillDock 命令行：开发、测试、发布 skill，并用软链接分发给各个 agent

用法：
  skl init [--root PATH]          创建根目录结构；--root 会写入 ~/.config/skilldock/config.toml
  skl root                        显示当前根目录
  skl new <name>                  在 dev/ 新建 skill 骨架
  skl ls [--agent codex]          按状态分组列出 skill；--agent 只看分发给某个 agent 的
  skl open <name>                 用 Finder 打开 skill 目录（优先开发区）
  skl show [name] [--released]    打印 SKILL.md（优先开发区）；在 skill 目录里可省略 name
  skl diff [name] [--stat]        开发区相对发布版改了什么；不带 name 列出所有有未发布改动的
  skl test <name> [--clean]       把开发版链到 _sandbox，测试自动触发
  skl publish <name> [--to a,b|all]
                                  发布：dev → released 快照，同步链接，git 提交。
                                  不带 --to：首次发布给全部 agent，更新沿用原分发
  skl link <name> <agent>...      追加分发（不重新发布内容），agent 可用 all
  skl unlink <name> <agent>...    移除分发（发布版保留）
  skl adopt <path> [--as name] [--to a,b] [--remove-source]
                                  把已有目录收为发布版（迁移/收录第三方）
  skl unpublish <name>            取消发布：删链接和 released/<name>，保留 dev/
  skl delete <name> [--yes]       删除 skill：取消发布，dev/<name> 移到 .trash/
  skl sync [--dry-run]            按 skills.toml 修正所有 agent 目录的链接
  skl doctor                      体检

根目录：SKILLDOCK_HOME > ~/.config/skilldock/config.toml 的 root > ~/.skilldock
";

struct Args {
    positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(raw: Vec<String>, with_value: &[&str]) -> Result<Self> {
        let mut positional = Vec::new();
        let mut flags = Vec::new();
        let mut it = raw.into_iter();
        while let Some(a) = it.next() {
            if let Some(f) = a.strip_prefix("--") {
                let (k, v) = match f.split_once('=') {
                    Some((k, v)) => (k.to_string(), Some(v.to_string())),
                    None if with_value.contains(&f) => {
                        let v = it.next().ok_or_else(|| Error(format!("--{f} 需要一个值")))?;
                        (f.to_string(), Some(v))
                    }
                    None => (f.to_string(), None),
                };
                flags.push((k, v));
            } else {
                positional.push(a);
            }
        }
        Ok(Args { positional, flags })
    }
    fn flag(&self, k: &str) -> bool {
        self.flags.iter().any(|(f, _)| f == k)
    }
    fn value(&self, k: &str) -> Option<String> {
        self.flags.iter().find(|(f, _)| f == k).and_then(|(_, v)| v.clone())
    }
    fn agents(&self) -> Option<Vec<String>> {
        self.value("to").map(|v| {
            v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        })
    }
    fn only(&self, allowed: &[&str]) -> Result<()> {
        for (f, _) in &self.flags {
            if !allowed.contains(&f.as_str()) {
                return Err(Error(format!("未知参数 --{f}")));
            }
        }
        Ok(())
    }
    fn one(&self, what: &str) -> Result<String> {
        match self.positional.as_slice() {
            [x] => Ok(x.clone()),
            _ => Err(Error(format!("需要一个参数：{what}"))),
        }
    }
}

fn print_sync(rep: &SyncReport, dry: bool) {
    let mut changed = 0;
    for it in &rep.items {
        let (mark, what) = match &it.action {
            Action::Kept => continue,
            Action::Created => ("+", "创建链接".to_string()),
            Action::Fixed(old) => ("~", format!("修正链接（原指向 {}）", tildify(old))),
            Action::Removed => ("-", "删除多余链接".to_string()),
            Action::Conflict(why) => ("!", format!("冲突，跳过：{why}")),
        };
        changed += 1;
        println!("  {mark} {:<10} {:<28} {what}", it.agent, it.name);
    }
    let kept = rep.items.iter().filter(|i| matches!(i.action, Action::Kept)).count();
    for n in &rep.notes {
        println!("  · {n}");
    }
    let prefix = if dry { "（预演，未改动）" } else { "" };
    println!("{prefix}变更 {changed} 项，保持 {kept} 项");
}

fn print_publish(rep: &PublishReport) {
    for w in &rep.warnings {
        println!("  ! {w}");
    }
    if !rep.agents.is_empty() {
        println!("  分发给：{}", rep.agents.join(", "));
    }
    print_sync(&rep.sync, false);
    if let Some(g) = &rep.git {
        println!("  {g}");
    }
}

fn run() -> Result<()> {
    let mut raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() || matches!(raw[0].as_str(), "-h" | "--help" | "help") {
        print!("{HELP}");
        return Ok(());
    }
    let cmd = raw.remove(0);
    let args = Args::parse(raw, &["root", "to", "as", "agent"])?;

    match cmd.as_str() {
        "init" => {
            args.only(&["root"])?;
            let root = match args.value("root") {
                Some(r) => {
                    let p = expand_tilde(&r)?;
                    let p = if p.is_absolute() { p } else { std::env::current_dir()?.join(p) };
                    let cfg = write_root_config(&p)?;
                    println!("根目录写入 {}", tildify(&cfg));
                    Root { path: p, source: RootSource::Config(cfg) }
                }
                None => resolve_root()?,
            };
            std::fs::create_dir_all(&root.path)?;
            let done = init(&root)?;
            if done.is_empty() {
                println!("{} 已就绪，无需改动", tildify(&root.path));
            }
            for d in done {
                println!("  {d}");
            }
            println!("下一步：检查 {} 里的 [agents]", tildify(&root.manifest_path()));
        }
        "root" => {
            let root = resolve_root()?;
            let src = match &root.source {
                RootSource::Env => "环境变量 SKILLDOCK_HOME".to_string(),
                RootSource::Config(p) => tildify(p),
                RootSource::Default => "默认值".to_string(),
            };
            println!("{}  （来自 {src}）", tildify(&root.path));
        }
        "new" => {
            args.only(&[])?;
            let dir = new_skill(&resolve_root()?, &args.one("name")?)?;
            println!("已创建 {}/SKILL.md", tildify(&dir));
        }
        "ls" | "list" => {
            args.only(&["agent"])?;
            let root = resolve_root()?;
            let mut list = status(&root)?;
            if let Some(a) = args.value("agent") {
                Manifest::load(&root)?.check_agents(std::slice::from_ref(&a))?;
                list.retain(|s| s.agents.contains(&a));
            }
            if list.is_empty() {
                println!("（没有 skill）");
            }
            let w = list.iter().map(|s| s.name.len()).max().unwrap_or(0).max(4);
            // Things needing attention first.
            let groups = [State::DevAhead, State::DevOnly, State::Released, State::ReleasedOnly, State::ManifestOnly];
            let mut first = true;
            for g in groups {
                let items: Vec<_> = list.iter().filter(|s| s.state == g).collect();
                if items.is_empty() {
                    continue;
                }
                if !first {
                    println!();
                }
                first = false;
                println!("{}（{}）", g.label(), items.len());
                for s in items {
                    let agents = if s.agents.is_empty() { "-".to_string() } else { s.agents.join(", ") };
                    println!("  {:<w$}  {agents}", s.name);
                }
            }
        }
        "open" => {
            args.only(&[])?;
            let path = skill_path(&resolve_root()?, &args.one("name")?)?;
            let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
            std::process::Command::new(opener).arg(&path).status()?;
            println!("{}", tildify(&path));
        }
        "show" | "cat" => {
            args.only(&["released"])?;
            let root = resolve_root()?;
            let name = match args.positional.as_slice() {
                [] => current_skill(&root)?,
                [x] => x.clone(),
                _ => return Err(Error("用法：skl show [name] [--released]".into())),
            };
            let dir = if args.flag("released") {
                check_name(&name)?;
                let d = root.released().join(&name);
                if !d.is_dir() {
                    return Err(Error(format!("`{name}` 还没有发布版")));
                }
                d
            } else {
                skill_path(&root, &name)?
            };
            let md = dir.join("SKILL.md");
            let text = std::fs::read_to_string(&md)
                .map_err(|e| Error(format!("读不了 {}：{e}", tildify(&md))))?;
            print!("{text}");
            if !text.ends_with('\n') {
                println!();
            }
        }
        "diff" => {
            args.only(&["stat"])?;
            let root = resolve_root()?;
            let name = match args.positional.as_slice() {
                [] => current_skill(&root).ok(),
                [x] => Some(x.clone()),
                _ => return Err(Error("用法：skl diff [name] [--stat]".into())),
            };
            let color = use_color();
            match name {
                Some(name) => {
                    let changes = diff_skill(&root, &name, !args.flag("stat"))?;
                    if changes.is_empty() {
                        println!("{name}：开发区和发布版一致");
                    } else {
                        print_changes(&name, &changes, !args.flag("stat"), color);
                    }
                }
                None => {
                    let ahead: Vec<_> =
                        status(&root)?.into_iter().filter(|s| matches!(s.state, State::DevAhead)).collect();
                    if ahead.is_empty() {
                        println!("没有未发布的改动");
                    }
                    for (i, s) in ahead.iter().enumerate() {
                        if i > 0 {
                            println!();
                        }
                        print_changes(&s.name, &diff_skill(&root, &s.name, false)?, false, color);
                    }
                    if !ahead.is_empty() {
                        println!("\n看具体内容：skl diff <name>");
                    }
                }
            }
        }
        "link" | "unlink" => {
            args.only(&[])?;
            let (name, agents) = match args.positional.split_first() {
                Some((n, rest)) if !rest.is_empty() => (n.clone(), rest.to_vec()),
                _ => return Err(Error(format!("用法：skl {cmd} <name> <agent>...（agent 可用 all）"))),
            };
            let root = resolve_root()?;
            let (now, rep) = if cmd == "link" { link(&root, &name, &agents)? } else { unlink(&root, &name, &agents)? };
            println!("{name} 现在分发给：{}", if now.is_empty() { "（无）".to_string() } else { now.join(", ") });
            print_sync(&rep, false);
        }
        "delete" | "rm" => {
            args.only(&["yes"])?;
            let name = args.one("name")?;
            let root = resolve_root()?;
            if !args.flag("yes") {
                eprint!("删除 {name}：取消发布并把 dev/{name} 移到 .trash/。确认？[y/N] ");
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if !matches!(answer.trim(), "y" | "Y" | "yes") {
                    println!("已取消");
                    return Ok(());
                }
            }
            let rep = delete(&root, &name)?;
            if let Some(sync) = &rep.unpublished {
                println!("已取消发布 {name}");
                print_sync(sync, false);
            }
            match rep.trashed {
                Some(p) => println!("dev/{name} 已移到 {}（需要时可移回 dev/）", tildify(&p)),
                None => println!("开发区没有 {name}，无需移动"),
            }
        }
        "test" => {
            args.only(&["clean"])?;
            let name = args.one("name")?;
            let root = resolve_root()?;
            let sb = test_link(&root, &name, args.flag("clean"))?;
            if args.flag("clean") {
                println!("已清理 _sandbox 中的 {name}");
            } else {
                println!("已把开发版 {name} 链到：");
                for d in SANDBOX_DIRS {
                    println!("  {}", tildify(&sb.join(d).join(&name)));
                }
                println!("在 {} 里启动 agent 测试自动触发；完成后 skl test {name} --clean", tildify(&sb));
            }
        }
        "publish" => {
            args.only(&["to"])?;
            let name = args.one("name")?;
            let rep = publish(&resolve_root()?, &name, args.agents())?;
            println!("已发布 {name}");
            print_publish(&rep);
        }
        "adopt" => {
            args.only(&["as", "to", "remove-source"])?;
            let src = PathBuf::from(args.one("path")?);
            let src = if src.is_absolute() { src } else { std::env::current_dir()?.join(src) };
            let (name, rep) = adopt(
                &resolve_root()?,
                &src,
                AdoptOptions { name: args.value("as"), to: args.agents(), remove_source: args.flag("remove-source") },
            )?;
            println!("已收编 {} → released/{name}", tildify(&src));
            print_publish(&rep);
        }
        "unpublish" => {
            args.only(&[])?;
            let name = args.one("name")?;
            let rep = unpublish(&resolve_root()?, &name)?;
            println!("已取消发布 {name}（dev/{name} 保留）");
            print_sync(&rep, false);
        }
        "sync" => {
            args.only(&["dry-run"])?;
            let dry = args.flag("dry-run");
            print_sync(&sync(&resolve_root()?, dry)?, dry);
        }
        "doctor" => {
            args.only(&[])?;
            let findings = doctor(&resolve_root()?)?;
            if findings.is_empty() {
                println!("一切正常");
            }
            for f in &findings {
                let m = match f.level {
                    Level::Error => "✗",
                    Level::Warn => "!",
                    Level::Info => "·",
                };
                println!("{m} {}", f.msg);
            }
            if findings.iter().any(|f| f.level == Level::Error) {
                return Err(Error("存在需要处理的问题".into()));
            }
        }
        other => return Err(Error(format!("未知命令 `{other}`，运行 skl --help 查看用法"))),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("skl: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The skill whose dev/ or released/ dir contains the current directory.
fn current_skill(root: &Root) -> Result<String> {
    let cwd = std::env::current_dir()?.canonicalize()?;
    for area in [root.dev(), root.released()] {
        if let Ok(area) = area.canonicalize() {
            if let Some(name) = cwd.strip_prefix(&area).ok().and_then(|r| r.components().next()) {
                return Ok(name.as_os_str().to_string_lossy().into_owned());
            }
        }
    }
    Err(Error("需要 skill 名字（或者在 skill 目录里运行）".into()))
}

fn use_color() -> bool {
    use std::io::IsTerminal;
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

fn paint(color: bool, code: &str, text: &str) -> String {
    if color { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
}

fn human_size(n: u64) -> String {
    match n {
        0..=1023 => format!("{n}B"),
        1024..=1_048_575 => format!("{:.1}KB", n as f64 / 1024.0),
        _ => format!("{:.1}MB", n as f64 / 1_048_576.0),
    }
}

/// File list (`+` added, `-` removed, `M` modified), then each modified file's patch.
fn print_changes(name: &str, changes: &[FileChange], patches: bool, color: bool) {
    println!("{}（{} 个文件）", paint(color, "1", name), changes.len());
    for c in changes {
        let (mark, code) = match c.kind {
            ChangeKind::Added => ("+", "32"),
            ChangeKind::Removed => ("-", "31"),
            ChangeKind::Modified => ("M", "33"),
        };
        let note = if c.binary { "  （二进制）" } else { "" };
        println!("  {} {}{note}", paint(color, code, mark), c.path);
    }
    if !patches {
        return;
    }
    for c in changes.iter().filter(|c| c.kind == ChangeKind::Modified) {
        println!();
        println!("{}", paint(color, "1", &format!("── {} ──", c.path)));
        match &c.patch {
            Some(patch) => {
                for line in patch.lines() {
                    let code = if line.starts_with("@@") {
                        "36"
                    } else if line.starts_with('+') {
                        "32"
                    } else if line.starts_with('-') {
                        "31"
                    } else {
                        ""
                    };
                    println!("{}", if code.is_empty() { line.to_string() } else { paint(color, code, line) });
                }
            }
            None => {
                let what = if c.binary { "二进制文件" } else { "未显示内容" };
                println!("{what}，大小 {} → {}", human_size(c.old_size), human_size(c.new_size));
            }
        }
    }
}
