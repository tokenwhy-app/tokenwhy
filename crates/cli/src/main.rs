// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

mod args;
mod color;
#[cfg(feature = "mcp")]
mod mcp;
mod run;
#[cfg(feature = "share")]
mod share;
#[cfg(feature = "wrapped")]
mod wrapped;

fn main() {
    #[cfg(feature = "wrapped")]
    if std::env::args().nth(1).as_deref() == Some("wrapped") {
        std::process::exit(wrapped::main(std::env::args().skip(2).collect()));
    }
    #[cfg(feature = "share")]
    if std::env::args().nth(1).as_deref() == Some("share") {
        std::process::exit(share::main(std::env::args().skip(2).collect()));
    }
    let parsed = if std::env::args().nth(1).as_deref() == Some("about") {
        Ok(args::Parsed::Print(about()))
    } else {
        args::parse(std::env::args().skip(1))
    };
    let code = match parsed {
        Ok(args::Parsed::Print(text)) => {
            use std::io::Write;
            match std::io::stdout().lock().write_all(text.as_bytes()) {
                Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => {
                    eprintln!("tokenwhy: {e}");
                    1
                }
                _ => 0,
            }
        }
        Ok(args::Parsed::Run(a)) => match dispatch(a) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("tokenwhy: {e}");
                1
            }
        },
        Err(e) => {
            eprintln!("tokenwhy: {e}\n\n{}", args::HELP);
            2
        }
    };
    std::process::exit(code);
}

fn dispatch(a: args::Args) -> Result<(), String> {
    if a.command == args::Command::Mcp {
        #[cfg(feature = "mcp")]
        return mcp::main(a);
        #[cfg(not(feature = "mcp"))]
        return Err("this build has no MCP server; rebuild with --features mcp".into());
    }
    run::run(a)
}

fn about() -> String {
    #[cfg(feature = "wrapped")]
    let extra = tokenwhy_wrapped::equiv::sources();
    #[cfg(not(feature = "wrapped"))]
    let extra = Vec::new();
    tokenwhy_core::about::text(&tokenwhy_core::about::sections(extra))
}
