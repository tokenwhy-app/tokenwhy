// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use lexopt::prelude::*;
use std::path::PathBuf;
use tokenwhy_core::history::By;
use tokenwhy_core::model::AnonLevel;
use tokenwhy_core::render::Format;

pub const HELP: &str = "\
tokenwhy: where your agent tokens go. Offline, read-only on transcripts.

USAGE
  tokenwhy <command> [options]

COMMANDS
  scan        read new transcript data into the cache and print file counts
  report      summary of every section, long lists capped, each with a +N more hint line
              (--full prints everything, --section <name> prints one section in full)
  startup     tokens loaded at the start of each session, per project
  tools       tool output size by command type
  findings    recommendations ranked by estimated tokens saved per week; each line has a
              rule code (R1 to R18) explained in the 'Rules:' line printed under the list
  live        totals for today and active sessions
  history     daily totals kept by tokenwhy, also for days whose transcripts are gone
  import ccusage <file>
              store daily totals from a ccusage JSON export (ccusage daily --by-agent --json);
              per agent and day tokenwhy's own data wins, the export fills the days it lacks;
              importing a file again replaces its rows
  report --for-agent
              short markdown brief for an agent: fixes ranked by $/week saved, each with its
              evidence and the exact instruction to apply (--format json for the JSON contract)
  mcp         local MCP server over stdio, read-only (built with the mcp feature); takes
              --window, --project, --provider, --anon and --root as defaults
  share preview | share export <file>
              the anonymous weekly summary the app sends when sharing is on, built
              offline for the last complete week; the CLI never sends it
  about       sources and credits: third-party code, price tables, CO2 factors, equivalents

OPTIONS (report, startup, tools, findings, live)
  --window <N>d|<N>h|all   period to analyse (default 14d)
  --project <name>         keep one project
  --provider <id>          keep one agent (e.g. claude-code)
  --anon[=strict]          replace project, session and path names with aliases;
                           strict also hides skill, agent type, MCP and memory file names
  --format text|md|json    output format (default text)
  --color auto|always|never  colour the text format (default auto: terminal only, NO_COLOR respected)

OPTIONS (report)
  --full                   print every row of every section
  --section <name>         one section in full: live, findings, startup, skills, memory,
                           subagents, compactions or tools

OPTIONS (history)
  --window <N>d|all        days to show (default 90d)
  --by project|model|provider   split each day by project (default), model or agent
  --project, --provider, --anon, --format as above

OPTIONS (all commands)
  --co2 ecologits|jegham       CO2e method (default ecologits: a range from output tokens only;
                               jegham: one value from all tokens, cache reads at a share of input)
  --co2-cache <factor>         jegham: energy of a cache read as a share of input (default 0.08)
  --co2-grid <g/kWh>           grid intensity instead of the method's (ecologits 384, jegham 287)
  --root [<provider>=]<path>   transcript root instead of the provider default (repeatable)
  --no-cache                   do not read or write the cache
  --stats                      print store sizes (files, sessions, RAM per item) to stderr
  -h, --help                   show this help
  -V, --version                show the version
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Scan,
    Report,
    Startup,
    Tools,
    Findings,
    Live,
    History,
    Import,
    Mcp,
}

impl Command {
    pub fn section(self) -> Option<&'static str> {
        match self {
            Command::Scan | Command::Report => None,
            Command::Startup => Some("startup"),
            Command::Tools => Some("tools"),
            Command::Findings => Some("findings"),
            Command::Live => Some("live"),
            Command::History | Command::Import | Command::Mcp => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowArg(pub Option<i64>);

pub const DAY_MS: i64 = 86_400_000;
pub const HOUR_MS: i64 = 3_600_000;

pub fn parse_window(s: &str) -> Result<WindowArg, String> {
    if s == "all" {
        return Ok(WindowArg(None));
    }
    let bad = || format!("bad window '{s}', use 14d, 36h or all");
    if s.len() < 2 || !s.is_char_boundary(s.len() - 1) {
        return Err(bad());
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let n: i64 = num.parse().map_err(|_| bad())?;
    if n <= 0 {
        return Err(bad());
    }
    match unit {
        "d" => Ok(WindowArg(Some(n * DAY_MS))),
        "h" => Ok(WindowArg(Some(n * HOUR_MS))),
        _ => Err(bad()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

pub fn parse_format(s: &str) -> Result<Format, String> {
    match s {
        "text" => Ok(Format::Text),
        "md" | "markdown" => Ok(Format::Markdown),
        "json" => Ok(Format::Json),
        _ => Err(format!("unknown format '{s}', use text, md or json")),
    }
}

#[derive(Debug)]
pub struct Args {
    pub command: Command,
    pub window: Option<WindowArg>,
    pub project: Option<String>,
    pub provider: Option<String>,
    pub anon: AnonLevel,
    pub format: Format,
    pub color: ColorMode,
    pub roots: Vec<(Option<String>, PathBuf)>,
    pub no_cache: bool,
    pub stats: bool,
    pub by: By,
    pub import: Option<PathBuf>,
    pub for_agent: bool,
    pub full: bool,
    pub section: Option<String>,
    #[cfg(feature = "co2")]
    pub co2: tokenwhy_core::co2::Co2Settings,
}

pub enum Parsed {
    Run(Args),
    Print(String),
}

fn val(p: &mut lexopt::Parser) -> Result<String, String> {
    p.value()
        .map_err(|e| e.to_string())?
        .string()
        .map_err(|e| e.to_string())
}

#[cfg(feature = "co2")]
fn number(s: &str) -> Result<f64, String> {
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| format!("bad number '{s}'"))
}

pub fn parse<I: IntoIterator<Item = String>>(argv: I) -> Result<Parsed, String> {
    let mut p = lexopt::Parser::from_args(argv);
    let mut command = None;
    let mut a = Args {
        command: Command::Report,
        window: None,
        project: None,
        provider: None,
        anon: AnonLevel::Off,
        format: Format::Text,
        color: ColorMode::Auto,
        roots: Vec::new(),
        no_cache: false,
        stats: false,
        by: By::Project,
        import: None,
        for_agent: false,
        full: false,
        section: None,
        #[cfg(feature = "co2")]
        co2: Default::default(),
    };
    let mut import_kind = None;
    while let Some(arg) = p.next().map_err(|e| e.to_string())? {
        match arg {
            Short('h') | Long("help") => return Ok(Parsed::Print(HELP.to_string())),
            Short('V') | Long("version") => {
                return Ok(Parsed::Print(format!(
                    "tokenwhy {}\n",
                    env!("CARGO_PKG_VERSION")
                )))
            }
            Long("window") => a.window = Some(parse_window(&val(&mut p)?)?),
            Long("project") => a.project = Some(val(&mut p)?),
            Long("provider") => a.provider = Some(val(&mut p)?),
            Long("anon") => {
                a.anon = match p.optional_value() {
                    None => AnonLevel::Names,
                    Some(v) if v == "strict" => AnonLevel::Strict,
                    Some(v) if v == "names" => AnonLevel::Names,
                    Some(v) => return Err(format!("bad --anon level: {}", v.to_string_lossy())),
                }
            }
            Long("format") => a.format = parse_format(&val(&mut p)?)?,
            Long("color") => {
                a.color = match val(&mut p)?.as_str() {
                    "auto" => ColorMode::Auto,
                    "always" => ColorMode::Always,
                    "never" => ColorMode::Never,
                    v => return Err(format!("unknown color '{v}', use auto, always or never")),
                }
            }
            Long("no-cache") => a.no_cache = true,
            Long("for-agent") => a.for_agent = true,
            Long("full") => a.full = true,
            Long("section") => {
                let v = val(&mut p)?;
                if !tokenwhy_core::render::SECTIONS.contains(&v.as_str()) {
                    return Err(format!(
                        "bad --section '{v}', use {}",
                        tokenwhy_core::render::SECTIONS.join(", ")
                    ));
                }
                a.section = Some(v);
            }
            Long("stats") => a.stats = true,
            Long("by") => {
                a.by = match val(&mut p)?.as_str() {
                    "project" => By::Project,
                    "model" => By::Model,
                    "provider" => By::Provider,
                    v => return Err(format!("bad --by '{v}', use project, model or provider")),
                }
            }
            #[cfg(feature = "co2")]
            Long("co2") => {
                use tokenwhy_core::co2::Method;
                a.co2.method = match val(&mut p)?.as_str() {
                    "ecologits" => Method::Ecologits,
                    "jegham" => Method::Jegham,
                    v => return Err(format!("bad --co2 '{v}', use ecologits or jegham")),
                }
            }
            #[cfg(feature = "co2")]
            Long("co2-cache") => a.co2.cache_read_factor = Some(number(&val(&mut p)?)?),
            #[cfg(feature = "co2")]
            Long("co2-grid") => a.co2.grid_g_per_kwh = Some(number(&val(&mut p)?)?),
            Long("root") => {
                let v = val(&mut p)?;
                match v.split_once('=') {
                    Some((id, path)) if !id.is_empty() => {
                        a.roots.push((Some(id.to_string()), path.into()))
                    }
                    _ => a.roots.push((None, v.into())),
                }
            }
            Value(v) if command == Some(Command::Import) && import_kind.is_none() => {
                let v = v.string().map_err(|e| e.to_string())?;
                if v != "ccusage" {
                    return Err(format!("unknown import '{v}', use: import ccusage <file>"));
                }
                import_kind = Some(v);
            }
            Value(v) if command == Some(Command::Import) && a.import.is_none() => {
                a.import = Some(v.into())
            }
            Value(v) if command.is_none() => {
                let v = v.string().map_err(|e| e.to_string())?;
                command = Some(match v.as_str() {
                    "scan" => Command::Scan,
                    "report" => Command::Report,
                    "startup" => Command::Startup,
                    "tools" => Command::Tools,
                    "findings" => Command::Findings,
                    "live" => Command::Live,
                    "history" => Command::History,
                    "import" => Command::Import,
                    "mcp" => Command::Mcp,
                    _ => return Err(format!("unknown command '{v}'")),
                });
            }
            other => return Err(other.unexpected().to_string()),
        }
    }
    a.command = command.ok_or_else(|| "missing command".to_string())?;
    if a.command == Command::Import && a.import.is_none() {
        return Err("usage: tokenwhy import ccusage <file>".into());
    }
    if a.for_agent && a.command != Command::Report {
        return Err("--for-agent goes with report".into());
    }
    if (a.full || a.section.is_some()) && a.command != Command::Report {
        return Err("--full and --section go with report".into());
    }
    if a.full && a.section.is_some() {
        return Err("--full and --section exclude each other".into());
    }
    Ok(Parsed::Run(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(v: &[&str]) -> Args {
        match parse(v.iter().map(|s| s.to_string())).unwrap() {
            Parsed::Run(a) => a,
            Parsed::Print(_) => panic!("expected a run"),
        }
    }

    #[test]
    fn window_units_in_milliseconds() {
        assert_eq!(
            parse_window("14d").unwrap(),
            WindowArg(Some(14 * 86_400_000))
        );
        assert_eq!(parse_window("36h").unwrap(), WindowArg(Some(129_600_000)));
        assert_eq!(parse_window("all").unwrap(), WindowArg(None));
        assert!(parse_window("0d").is_err());
        assert!(parse_window("d").is_err());
        assert!(parse_window("5x").is_err());
    }

    #[test]
    fn report_options() {
        let a = run(&[
            "report",
            "--window",
            "7d",
            "--project",
            "demo",
            "--anon",
            "--format",
            "md",
        ]);
        assert_eq!(a.command, Command::Report);
        assert_eq!(a.window, Some(WindowArg(Some(7 * 86_400_000))));
        assert_eq!(a.project.as_deref(), Some("demo"));
        assert_eq!(a.anon, AnonLevel::Names);
        assert_eq!(a.format, Format::Markdown);
    }

    #[test]
    fn root_with_and_without_provider_id() {
        let a = run(&["scan", "--root", "claude-code=D:\\fx", "--root", "C:\\bare"]);
        assert_eq!(
            a.roots[0],
            (Some("claude-code".into()), PathBuf::from("D:\\fx"))
        );
        assert_eq!(a.roots[1], (None, PathBuf::from("C:\\bare")));
    }

    #[test]
    fn errors() {
        assert!(parse(["bogus".to_string()]).is_err());
        assert!(parse(Vec::<String>::new()).is_err());
        assert!(parse(["report".to_string(), "--format".into(), "xml".into()]).is_err());
    }
}
