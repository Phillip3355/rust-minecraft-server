//! datapack-audit --pumpkin-src <dir> --pack <dir|zip> [--pack ...] [--json-out F] [--md-out F] [--rev R]
//!
//! Exit status: 0 = ran (findings are data, not errors), 2 = usage / source-extraction error.

use std::path::PathBuf;
use std::process::ExitCode;

use datapack_audit::{audit, caps, report};

fn main() -> ExitCode {
    let mut src = None;
    let mut packs = Vec::new();
    let mut json_out = None;
    let mut md_out = None;
    let mut rev = String::from("eda69440");
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().map(PathBuf::from);
        match a.as_str() {
            "--pumpkin-src" => src = val(),
            "--pack" => packs.extend(val()),
            "--json-out" => json_out = val(),
            "--md-out" => md_out = val(),
            "--rev" => rev = val().map(|p| p.display().to_string()).unwrap_or(rev),
            "-h" | "--help" => {
                println!("datapack-audit --pumpkin-src <dir> --pack <dir> [--pack <dir>...] [--json-out F] [--md-out F] [--rev R]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown argument: {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(src) = src else {
        eprintln!("--pumpkin-src is required");
        return ExitCode::from(2);
    };
    if packs.is_empty() {
        eprintln!("at least one --pack is required");
        return ExitCode::from(2);
    }
    let cap = match caps::load(&src) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("capability extraction failed: {e}");
            return ExitCode::from(2);
        }
    };
    let results: Vec<_> = packs.iter().map(|p| audit::audit_pack(&audit::Pack::open(p), &cap)).collect();
    for r in &results {
        let t = report::totals(r);
        let s: Vec<String> = t.iter().map(|(k, (d, c))| format!("{k}={d} distinct/{c} occurrences")).collect();
        println!("{}: {}", r.name, s.join(", "));
    }
    for (path, body) in [(json_out, report::to_json(&cap, &results, &rev)), (md_out, report::to_markdown(&cap, &results, &rev))] {
        if let Some(p) = path {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            if let Err(e) = std::fs::write(&p, body) {
                eprintln!("write {}: {e}", p.display());
                return ExitCode::from(2);
            }
        }
    }
    ExitCode::SUCCESS
}
