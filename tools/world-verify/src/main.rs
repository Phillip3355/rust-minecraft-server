use std::path::PathBuf;
use std::process::ExitCode;

use world_verify::nbt::Tag;
use world_verify::report::{Category, Report};
use world_verify::{fixture, items, world};

const USAGE: &str = "\
world-verify — Phase 1 Anvil world integrity verifier (offline, no dependencies)

USAGE:
  world-verify verify   <world>                 [--expect-data-version N] [--json FILE]
  world-verify diff     <before> <after>        [--strict] [--json FILE]
  world-verify cleanroom <generated-world> <reloaded-world> [--json FILE]
  world-verify items    <world>                 [--json FILE]
  world-verify conserve <before> <after>        [--allow ITEM_ID]... [--strict-loss] [--json FILE]
  world-verify fixture  <dir>                   [--compression 1|2|3|4]

verify    structural check: region headers/sectors, compression checksums, NBT,
          chunk xPos/zPos, palettes/packed arrays, block entities, level.dat,
          playerdata (Pos/Health/slots/stack counts).
diff      snapshot comparison: lost chunks, lost players; with --strict (no
          gameplay between snapshots) also changed blocks/block entities and
          changed inventories/positions.
cleanroom checks expanded sections, terrain and underground samples, and a strict reload diff.
items     item totals across players, containers, item entities.
conserve  item conservation: any increase = duplication (error); decrease =
          possible loss (warning, or error with --strict-loss).
fixture   write a small synthetic world (verifier self-test only).

EXIT: 0 = no errors, 1 = errors found, 2 = usage/IO failure";

struct Args {
    pos: Vec<String>,
    json: Option<PathBuf>,
    strict: bool,
    strict_loss: bool,
    allow: Vec<String>,
    expect_dv: Option<i64>,
    compression: u8,
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut a = Args { pos: vec![], json: None, strict: false, strict_loss: false, allow: vec![], expect_dv: None, compression: 2 };
    let mut it = argv.iter();
    while let Some(x) = it.next() {
        let mut val = |name: &str| it.next().cloned().ok_or(format!("{name} needs a value"));
        match x.as_str() {
            "--json" => a.json = Some(PathBuf::from(val("--json")?)),
            "--strict" => a.strict = true,
            "--strict-loss" => a.strict_loss = true,
            "--allow" => a.allow.push(val("--allow")?),
            "--expect-data-version" => {
                a.expect_dv = Some(val("--expect-data-version")?.parse().map_err(|_| "bad --expect-data-version")?)
            }
            "--compression" => {
                a.compression = val("--compression")?.parse().map_err(|_| "bad --compression")?;
                if !(1..=4).contains(&a.compression) {
                    return Err("--compression must be 1..4".into());
                }
            }
            "-h" | "--help" => return Err(String::new()),
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            _ => a.pos.push(x.clone()),
        }
    }
    Ok(a)
}

fn prefixed(mut r: Report, p: &str) -> Report {
    for i in &mut r.issues {
        i.location = format!("[{p}] {}", i.location);
    }
    r
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = argv.first().cloned() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let args = match parse(&argv[1..]) {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("error: {e}\n");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let need = |n: usize| -> bool {
        if args.pos.len() != n {
            eprintln!("error: '{cmd}' expects {n} path argument(s)\n\n{USAGE}");
            false
        } else {
            true
        }
    };

    let mut report = Report::default();
    match cmd.as_str() {
        "verify" => {
            if !need(1) {
                return ExitCode::from(2);
            }
            let w = world::load(PathBuf::from(&args.pos[0]).as_path());
            if let Some(dv) = args.expect_dv {
                for (dim, chunks) in &w.terrain {
                    for ((cx, cz), ch) in chunks {
                        let got = ch.nbt.get("DataVersion").and_then(Tag::as_i64);
                        if got.is_some_and(|g| g != dv) {
                            report.error(
                                Category::ChunkStructure,
                                format!("{dim} chunk({cx},{cz})"),
                                format!("DataVersion {} != expected {dv}", got.unwrap()),
                            );
                        }
                    }
                }
            }
            report.merge(w.report);
        }
        "diff" | "conserve" | "cleanroom" => {
            if !need(2) {
                return ExitCode::from(2);
            }
            let mut a = world::load(PathBuf::from(&args.pos[0]).as_path());
            let mut b = world::load(PathBuf::from(&args.pos[1]).as_path());
            if cmd == "cleanroom" {
                world_verify::cleanroom::verify(&a, &mut report);
                world::diff(&a, &b, true, &mut report);
            } else if cmd == "diff" {
                world::diff(&a, &b, args.strict, &mut report);
            } else {
                let ta = a.item_totals();
                let tb = b.item_totals();
                items::conserve(&ta, &tb, &args.allow, args.strict_loss, &mut report);
            }
            report.merge(prefixed(std::mem::take(&mut a.report), "before"));
            report.merge(prefixed(std::mem::take(&mut b.report), "after"));
        }
        "items" => {
            if !need(1) {
                return ExitCode::from(2);
            }
            let mut w = world::load(PathBuf::from(&args.pos[0]).as_path());
            let totals = w.item_totals();
            for (id, n) in &totals.by_id {
                println!("{n:>8}  {id}");
                report.bump(&format!("item:{id}"), *n);
            }
            report.merge(std::mem::take(&mut w.report));
        }
        "fixture" => {
            if !need(1) {
                return ExitCode::from(2);
            }
            if let Err(e) = fixture::write_world(PathBuf::from(&args.pos[0]).as_path(), args.compression) {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
            println!("wrote synthetic fixture world to {}", args.pos[0]);
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("unknown command {cmd}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    }

    print!("{}", report.to_text(200));
    if let Some(p) = &args.json {
        if let Err(e) = std::fs::write(p, report.to_json(&cmd)) {
            eprintln!("error: cannot write {}: {e}", p.display());
            return ExitCode::from(2);
        }
    }
    let io_fail = report.issues.iter().any(|i| i.category == Category::Io && i.message.contains("world directory does not exist"));
    if io_fail {
        ExitCode::from(2)
    } else if report.errors() > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
