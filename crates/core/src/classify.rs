// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Rules rewritten from the ideas of codeburn bash-utils.ts; see NOTICE.

use crate::model::Category;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    pub category: Category,
    pub detail: Option<String>,
    pub bounded: bool,
    pub path: Option<String>,
}

impl Classified {
    fn new(category: Category) -> Self {
        Self {
            category,
            detail: None,
            bounded: false,
            path: None,
        }
    }
    fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }
}

pub fn is_log_path(path: &str) -> bool {
    let p = path.replace('\\', "/").to_lowercase();
    p.ends_with(".log") || p.ends_with(".out") || p.split('/').any(|c| c == "logs")
}

struct Seg {
    words: Vec<String>,
    piped: bool,
}

#[allow(unused_assignments)]
fn split_segments(cmd: &str) -> Vec<Seg> {
    let mut segs: Vec<Seg> = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut has_word = false;
    let mut quote: Option<char> = None;
    let mut piped = false;
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;

    macro_rules! end_word {
        () => {
            if has_word {
                words.push(std::mem::take(&mut cur));
                has_word = false;
            }
        };
    }
    macro_rules! end_seg {
        ($next_piped:expr) => {
            end_word!();
            if !words.is_empty() {
                segs.push(Seg {
                    words: std::mem::take(&mut words),
                    piped,
                });
            }
            piped = $next_piped;
        };
    }

    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else if q == '"' && c == '\\' && chars.get(i + 1) == Some(&'"') {
                cur.push('"');
                i += 1;
            } else {
                cur.push(c);
            }
            i += 1;
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                has_word = true;
            }
            c if c.is_whitespace() && c != '\n' => end_word!(),
            '\n' | ';' => {
                end_seg!(false);
            }
            '&' if chars.get(i + 1) == Some(&'&') => {
                end_seg!(false);
                i += 1;
            }
            '|' if chars.get(i + 1) == Some(&'|') => {
                end_seg!(false);
                i += 1;
            }
            '|' => {
                end_seg!(true);
            }
            // PowerShell call operator `& "tool"` or a trailing background `&`; `2>&1` keeps its `&` inside the word.
            '&' if !has_word && chars.get(i + 1).is_none_or(|n| n.is_whitespace()) => {}
            _ => {
                cur.push(c);
                has_word = true;
            }
        }
        i += 1;
    }
    end_seg!(false);
    segs
}

fn prog_name(word: &str) -> String {
    let base = word
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(word)
        .to_lowercase();
    for suf in [".exe", ".cmd", ".bat"] {
        if let Some(s) = base.strip_suffix(suf) {
            return s.to_string();
        }
    }
    base
}

fn is_assignment(w: &str) -> bool {
    match w.split_once('=') {
        Some((k, _)) => {
            !k.is_empty()
                && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !k.starts_with(|c: char| c.is_ascii_digit())
        }
        None => false,
    }
}

fn is_script_statement(w: &str) -> bool {
    let p = w.to_lowercase();
    p.starts_with('$')
        || p.contains("()")
        || matches!(
            p.split('(').next().unwrap_or(""),
            "for"
                | "foreach"
                | "while"
                | "until"
                | "if"
                | "do"
                | "function"
                | "case"
                | "select"
                | "switch"
                | "try"
                | "{"
        )
}

fn is_redirect(w: &str) -> bool {
    let t = w.trim_start_matches(|c: char| c.is_ascii_digit());
    t.starts_with('>') || t.starts_with("&>") || (w.starts_with('<') && w.len() > 1)
}

fn strip_prefixes(words: &[String]) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let mut skip_next = false;
    for w in words {
        if skip_next {
            skip_next = false;
            continue;
        }
        if w == ">" || w == ">>" || w == "2>" || w == "1>" || w == "<" {
            skip_next = true;
            continue;
        }
        if is_redirect(w) {
            continue;
        }
        v.push(w.clone());
    }
    let mut i = 0;
    while i < v.len() {
        let w = &v[i];
        if is_assignment(w) {
            let value = w.split_once('=').map_or("", |(_, v)| v);
            let inner = value.strip_prefix("$(").or_else(|| value.strip_prefix('`'));
            match inner {
                Some(cmd) if !cmd.is_empty() => {
                    v[i] = cmd.to_string();
                    break;
                }
                Some(_) => {
                    i += 1;
                    break;
                }
                None => {
                    i += 1;
                    continue;
                }
            }
        }
        let p = prog_name(w);
        if matches!(
            p.as_str(),
            "sudo" | "npx" | "time" | "nice" | "nohup" | "env" | "command" | "exec" | "call"
        ) {
            i += 1;
            while i < v.len() && v[i].starts_with('-') && v[i].len() > 1 {
                let value_follows = p == "nice" && v[i] == "-n";
                i += 1;
                if value_follows {
                    i += 1;
                }
            }
            continue;
        }
        break;
    }
    v.split_off(i)
}

fn positionals<'a>(args: &'a [String], value_flags: &[&str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let lower = a.to_lowercase();
        if value_flags.contains(&lower.as_str()) {
            i += 2;
            continue;
        }
        let numeric_flag =
            a.starts_with('-') && a.len() > 1 && a[1..].chars().all(|c| c.is_ascii_digit());
        if (a.starts_with('-') && a.len() > 1)
            || numeric_flag
            || (a.starts_with('/') && a.len() == 2)
        {
            i += 1;
            continue;
        }
        out.push(a);
        i += 1;
    }
    out
}

fn flag_value<'a>(args: &'a [String], names: &[&str]) -> Option<&'a str> {
    args.iter().enumerate().find_map(|(i, a)| {
        names
            .contains(&a.to_lowercase().as_str())
            .then(|| args.get(i + 1).map(|s| s.as_str()))
            .flatten()
    })
}

fn has_flag(args: &[String], names: &[&str]) -> bool {
    args.iter()
        .any(|a| names.contains(&a.to_lowercase().as_str()))
}

fn short(s: &str) -> String {
    s.chars().take(20).collect()
}

fn file_stem(w: &str) -> String {
    let base = w.rsplit(['/', '\\']).next().unwrap_or(w);
    match base.rsplit_once('.') {
        Some((s, _)) if !s.is_empty() => s.to_lowercase(),
        _ => base.to_lowercase(),
    }
}

fn read_result(path: Option<&str>, bounded: bool) -> Classified {
    let cat = if path.is_some_and(is_log_path) {
        Category::Logs
    } else {
        Category::ReadFile
    };
    let mut c = Classified::new(cat);
    c.bounded = bounded;
    c.path = path.map(str::to_string);
    c
}

fn classify_words(words: &[String], project_tools: &[String], depth: u32) -> Classified {
    let prog = prog_name(&words[0]);
    let args = &words[1..];

    for t in project_tools {
        if prog == t.to_lowercase() {
            return Classified::new(Category::ProjectTool).detail(t.clone());
        }
    }

    match prog.as_str() {
        "head" | "tail" => {
            let pos = positionals(args, &["-n", "-c"]);
            read_result(pos.first().copied(), true)
        }
        "cat" | "type" | "bat" | "less" | "more" => {
            let pos = positionals(args, &[]);
            read_result(pos.first().copied(), false)
        }
        "sed" => {
            if args.iter().any(|a| {
                a == "-i" || a.starts_with("-i") && !a.starts_with("-in") || a == "--in-place"
            }) {
                return Classified::new(Category::Other).detail("sed");
            }
            let has_script_flag = has_flag(args, &["-e", "-f"]);
            let pos = positionals(args, &["-e", "-f"]);
            let path = if has_script_flag {
                pos.first()
            } else {
                pos.get(1)
            }
            .copied();
            read_result(path, true)
        }
        "get-content" | "gc" => {
            let path = flag_value(args, &["-path", "-literalpath"]).or_else(|| {
                positionals(
                    args,
                    &[
                        "-totalcount",
                        "-tail",
                        "-head",
                        "-first",
                        "-last",
                        "-encoding",
                        "-readcount",
                        "-delimiter",
                        "-filter",
                        "-include",
                        "-exclude",
                    ],
                )
                .first()
                .copied()
            });
            let bounded = has_flag(args, &["-totalcount", "-tail", "-head", "-first", "-last"]);
            read_result(path, bounded)
        }
        "grep" | "egrep" | "fgrep" | "rg" | "ag" | "ack" | "findstr" | "select-string" | "sls" => {
            let value_flags: &[&str] = &[
                "-e",
                "-f",
                "-m",
                "-a",
                "-b",
                "-c",
                "-g",
                "--glob",
                "-t",
                "--type",
                "-pattern",
                "--include",
                "--exclude",
            ];
            let path = if prog == "select-string" || prog == "sls" {
                flag_value(args, &["-path", "-literalpath"]).or_else(|| {
                    positionals(args, &["-pattern", "-path", "-context"])
                        .get(1)
                        .copied()
                })
            } else {
                let has_e = has_flag(args, &["-e"]);
                let pos = positionals(args, value_flags);
                if has_e {
                    pos.first().copied()
                } else {
                    pos.get(1).copied()
                }
            };
            let mut c = Classified::new(if path.is_some_and(is_log_path) {
                Category::Logs
            } else {
                Category::Search
            });
            c.path = path.map(str::to_string);
            c
        }
        "ls" | "ll" | "dir" | "find" | "tree" | "get-childitem" | "gci" | "fd" | "fdfind" => {
            Classified::new(Category::List)
        }
        "git" => {
            let mut i = 0;
            while i < args.len() {
                match args[i].as_str() {
                    "-C" | "-c" | "--git-dir" | "--work-tree" => i += 2,
                    a if a.starts_with('-') => i += 1,
                    _ => break,
                }
            }
            let mut c = Classified::new(Category::Vcs);
            c.detail = args.get(i).cloned();
            c
        }
        "gh" => Classified::new(Category::Vcs).detail("gh"),
        "gradle" | "gradlew" => {
            let test = args.iter().any(|a| {
                let t = a.rsplit(':').next().unwrap_or(a);
                t == "test" || t == "check" || t.starts_with("test") && !t.starts_with('-')
            });
            Classified::new(if test {
                Category::Test
            } else {
                Category::Build
            })
        }
        "mvn" | "mvnw" => {
            let test = args.iter().any(|a| a == "test" || a == "verify");
            Classified::new(if test {
                Category::Test
            } else {
                Category::Build
            })
        }
        "cargo" => {
            let pos = positionals(args, &[]);
            let sub = pos
                .iter()
                .find(|p| !p.starts_with('+'))
                .copied()
                .unwrap_or("");
            match sub {
                "test" | "bench" | "nextest" => Classified::new(Category::Test),
                "build" | "check" | "clippy" | "doc" | "fmt" | "run" | "install" | "b" | "c"
                | "r" => Classified::new(Category::Build),
                _ => Classified::new(Category::Other).detail("cargo"),
            }
        }
        "npm" | "pnpm" | "yarn" => {
            let pos = positionals(args, &[]);
            let sub = pos.first().copied().unwrap_or("");
            let script = if matches!(sub, "run" | "run-script") {
                pos.get(1).copied().unwrap_or("")
            } else {
                sub
            };
            if script == "test" || script == "t" || script.starts_with("test") {
                Classified::new(Category::Test)
            } else if script.starts_with("build")
                || script.starts_with("compile")
                || script == "tsc"
                || script.starts_with("lint")
            {
                Classified::new(Category::Build)
            } else {
                Classified::new(Category::Other).detail(prog.clone())
            }
        }
        "tsc" | "make" | "ninja" | "cmake" | "msbuild" | "xcodebuild" => {
            Classified::new(Category::Build)
        }
        "dotnet" => match args.first().map(String::as_str) {
            Some("test") => Classified::new(Category::Test),
            Some("build" | "publish" | "run") => Classified::new(Category::Build),
            _ => Classified::new(Category::Other).detail("dotnet"),
        },
        "go" => match args.first().map(String::as_str) {
            Some("test") => Classified::new(Category::Test),
            Some("build" | "vet" | "install") => Classified::new(Category::Build),
            _ => Classified::new(Category::Other).detail("go"),
        },
        "pytest" | "py.test" | "jest" | "vitest" | "tox" | "phpunit" => {
            Classified::new(Category::Test)
        }
        "journalctl" => Classified::new(Category::Logs).detail("journalctl"),
        "docker" | "podman" | "kubectl" => {
            if args.iter().any(|a| a == "logs") {
                Classified::new(Category::Logs).detail(format!("{prog} logs"))
            } else {
                Classified::new(Category::Other).detail(prog.clone())
            }
        }
        "curl" | "wget" | "invoke-webrequest" | "iwr" | "invoke-restmethod" | "irm" | "http" => {
            Classified::new(Category::Web)
        }
        p if p == "python"
            || p == "python3"
            || p == "py"
            || p.starts_with("python3.")
            || p == "node"
            || p == "deno"
            || p == "ruby"
            || p == "perl" =>
        {
            if has_flag(args, &["-m"]) {
                let m = flag_value(args, &["-m"]).unwrap_or("");
                return if matches!(m, "pytest" | "unittest") {
                    Classified::new(Category::Test)
                } else {
                    Classified::new(Category::Script).detail(m)
                };
            }
            let pos = positionals(args, &["-c", "-e", "--eval"]);
            let pos: Vec<&str> = pos.into_iter().filter(|p| *p != "run").collect();
            match pos.first().filter(|s| **s != "-") {
                Some(script) => {
                    let stem = file_stem(script);
                    for t in project_tools {
                        if stem == t.to_lowercase() {
                            return Classified::new(Category::ProjectTool).detail(t.clone());
                        }
                    }
                    let base = script.rsplit(['/', '\\']).next().unwrap_or(script);
                    Classified::new(Category::Script).detail(base)
                }
                None => Classified::new(Category::Script),
            }
        }
        "bash" | "sh" | "zsh" | "pwsh" | "powershell" => {
            let cmd_flag = args.iter().position(|a| {
                let l = a.to_lowercase();
                l == "-command"
                    || l == "-c"
                    || (a.starts_with('-')
                        && !a.starts_with("--")
                        && a.len() <= 4
                        && a.ends_with('c')
                        && a.len() > 1
                        && prog != "pwsh"
                        && prog != "powershell")
            });
            if let (Some(i), true) = (cmd_flag, depth < 3) {
                if let Some(inner) = args.get(i + 1) {
                    return classify_str(inner, project_tools, depth + 1);
                }
            }
            if let Some(f) = flag_value(args, &["-file"]) {
                return Classified::new(Category::Script)
                    .detail(f.rsplit(['/', '\\']).next().unwrap_or(f));
            }
            let pos = positionals(args, &[]);
            match pos.first().filter(|s| **s != "-") {
                Some(script) => Classified::new(Category::Script)
                    .detail(script.rsplit(['/', '\\']).next().unwrap_or(script)),
                None => Classified::new(Category::Other).detail(short(&prog)),
            }
        }
        _ => Classified::new(Category::Other).detail(short(&prog)),
    }
}

fn classify_str(command: &str, project_tools: &[String], depth: u32) -> Classified {
    let segs = split_segments(command);
    let bounded_pipe = segs.iter().any(|s| {
        s.piped && {
            let w = strip_prefixes(&s.words);
            w.first()
                .is_some_and(|p| matches!(prog_name(p).as_str(), "head" | "tail"))
        }
    });
    for seg in segs.iter().filter(|s| !s.piped) {
        let words = strip_prefixes(&seg.words);
        let Some(first) = words.first() else { continue };
        if matches!(
            prog_name(first).as_str(),
            "cd" | "pushd"
                | "popd"
                | "set-location"
                | "sl"
                | "export"
                | "set"
                | "echo"
                | "true"
                | "source"
        ) {
            continue;
        }
        if is_script_statement(first) || is_script_statement(&prog_name(first)) {
            return Classified::new(Category::Script);
        }
        let mut c = classify_words(&words, project_tools, depth);
        c.bounded |= bounded_pipe;
        return c;
    }
    Classified::new(Category::Other)
}

pub fn classify_command(command: &str, project_tools: &[String]) -> Classified {
    classify_str(command, project_tools, 0)
}
