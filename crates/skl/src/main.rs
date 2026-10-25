use skilldock_core::*;
use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "skl —— SkillDock 命令行：开发、测试、发布 skill，并用软链接分发给各个 agent

用法：
  skl init [--root PATH]          创建根目录结构；--root 会写入 ~/.config/skilldock/config.toml
  skl root                        显示当前根目录
  skl new <name>                  在 dev/ 新建 skill 骨架
  skl ls                          列出所有 skill 的状态和分发情况
  skl test <name> [--clean]       把开发版链到 _sandbox，测试自动触发
  skl publish <name> [--to a,b]   发布：dev → released 快照，同步链接，git 提交
  skl adopt <path> [--as name] [--to a,b] [--remove-source]
                                  把已有目录收为发布版（迁移/收录第三方）
  skl unpublish <name>            取消发布：删链接和 released/<name>，保留 dev/
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

/// Terminal columns: CJK characters take two.
fn display_width(s: &str) -> usize {
    s.chars().map(|c| if (c as u32) >= 0x1100 { 2 } else { 1 }).sum()
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
    let args = Args::parse(raw, &["root", "to", "as"])?;

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
            args.only(&[])?;
            let list = status(&resolve_root()?)?;
            if list.is_empty() {
                println!("（没有 skill）");
            }
            let w = list.iter().map(|s| s.name.len()).max().unwrap_or(0).max(4);
            for s in list {
                let agents = if s.agents.is_empty() { "-".to_string() } else { s.agents.join(",") };
                let label = s.state.label();
                println!("{:<w$}  {label}{}  {agents}", s.name, " ".repeat(18usize.saturating_sub(display_width(label))));
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
