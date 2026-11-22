// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use tokenwhy_core::classify::classify_command;
use tokenwhy_core::Category::{self, *};

fn tools() -> Vec<String> {
    vec!["codeq".into(), "logq".into()]
}

#[test]
fn classify() {
    let cases: &[(&str, Category, bool)] = &[
        ("sed -n 120,240p F.java", ReadFile, true),
        ("cd x && grep -rn foo src | head", Search, true),
        ("FOO=1 npx tsc", Build, false),
        ("git diff | head -50", Vcs, true),
        ("tail -200 logs/latest.log", Logs, true),
        (
            "python D:/CODE_PROJECTS/KNOWLEDGE_BASE/tools/codeq/codeq.py outline --root r",
            ProjectTool,
            false,
        ),
        ("Get-Content f -TotalCount 50", ReadFile, true),
        ("cat README.md", ReadFile, false),
        ("grep -n ERROR build/out.log", Logs, false),
        ("rg -n \"a|b\" src", Search, false),
        ("ls -la", List, false),
        ("find . -name '*.rs'", List, false),
        ("Get-ChildItem -Recurse src", List, false),
        ("git -C repo status", Vcs, false),
        ("./gradlew.bat test --no-daemon", Test, false),
        ("./gradlew build", Build, false),
        ("cargo test -p core", Test, false),
        ("cargo build --release", Build, false),
        ("npm run build", Build, false),
        ("npm test", Test, false),
        ("pytest -q tests", Test, false),
        ("python tools/gen.py --x", Script, false),
        ("sudo nice -n 5 time make -j4", Build, false),
        ("curl -s https://example.com", Web, false),
        ("bash -c \"cd d && cargo check\"", Build, false),
        (
            "pwsh -NoProfile -Command \"Get-Content a.log -Tail 20\"",
            Logs,
            true,
        ),
        ("sed -i 's/a/b/' f.txt", Other, false),
        ("echo done", Other, false),
    ];
    for (cmd, cat, bounded) in cases {
        let c = classify_command(cmd, &tools());
        assert_eq!((c.category, c.bounded), (*cat, *bounded), "{cmd}");
    }
    assert_eq!(
        classify_command("git diff | head -50", &tools())
            .detail
            .as_deref(),
        Some("diff")
    );
    assert_eq!(
        classify_command("python tools/gen.py --x", &tools())
            .detail
            .as_deref(),
        Some("gen.py")
    );
    assert_eq!(
        classify_command("python x/codeq.py map", &tools())
            .detail
            .as_deref(),
        Some("codeq")
    );
    assert_eq!(
        classify_command("sed -n 1,5p src/A.java", &tools())
            .path
            .as_deref(),
        Some("src/A.java")
    );
    assert_eq!(
        classify_command("head -n 20 a.txt", &tools())
            .path
            .as_deref(),
        Some("a.txt")
    );
}

#[test]
fn script_fragments_are_not_tool_names() {
    let cases: &[(&str, Category, Option<&str>)] = &[
        ("$j=@(Get-CimInstance Win32_Process -Filter \"name like 'java%'\").Count; $o=Get-CimInstance Win32_OperatingSystem", Script, None),
        ("$R='D:/x/tests/1.16.5-fabric_1.4.0_final'; Get-ChildItem $R -Name", Script, None),
        ("cd /d/x/_temp/mineai; \ntag(){ sed -i \"s/a/b/\" \"$2\"; }\ntag S f.md", Script, None),
        ("for f in a b; do cat $f; done", Script, None),
        ("source lib.sh; M=$(modsdir 1.20.1-fabric_1.4.0_final); ls $M", Other, Some("modsdir")),
    ];
    for (cmd, cat, detail) in cases {
        let c = classify_command(cmd, &[]);
        assert_eq!((c.category, c.detail.as_deref()), (*cat, *detail), "{cmd}");
    }
}
