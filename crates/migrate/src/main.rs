//! CLI entry point for `cordex-migrate`.

use std::path::PathBuf;
use std::process::ExitCode;

use cordex_migrate::{ApplyOptions, CORDEX_VERSION, analyze, apply};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let Some(cmd) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };

    match cmd {
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        "-V" | "--version" | "version" => {
            println!("cordex-migrate {CORDEX_VERSION}");
            Ok(())
        }
        "analyze" => run_analyze(&args[1..]),
        "apply" => run_apply(&args[1..]),
        other => Err(format!(
            "unknown subcommand `{other}` (run `cordex-migrate --help`)"
        )),
    }
}

fn run_analyze(args: &[String]) -> Result<(), String> {
    let mut target: Option<PathBuf> = None;

    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            flag if flag.starts_with('-') => {
                return Err(format!("unknown flag `{flag}` for `analyze`"));
            }
            path => {
                if target.is_some() {
                    return Err("expected at most one path argument".to_owned());
                }
                target = Some(PathBuf::from(path));
            }
        }
    }

    let root = target.unwrap_or_else(|| PathBuf::from("."));
    let report = analyze(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    print!("{}", report.format_text());
    Ok(())
}

fn run_apply(args: &[String]) -> Result<(), String> {
    let mut target: Option<PathBuf> = None;
    let mut options = ApplyOptions::default();

    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "--dry-run" => options.write = false,
            "--write" => options.write = true,
            "--no-todos" => options.annotate_todos = false,
            flag if flag.starts_with('-') => {
                return Err(format!("unknown flag `{flag}` for `apply`"));
            }
            path => {
                if target.is_some() {
                    return Err("expected at most one path argument".to_owned());
                }
                target = Some(PathBuf::from(path));
            }
        }
    }

    let root = target.unwrap_or_else(|| PathBuf::from("."));
    let report = apply(&root, options).map_err(|e| format!("{}: {e}", root.display()))?;
    print!("{}", report.format_text());
    Ok(())
}

fn print_help() {
    println!(
        "\
cordex-migrate {CORDEX_VERSION}
Analyze and mechanically migrate Serenity, Poise, and Twilight bots to cordex.

USAGE:
    cordex-migrate analyze [PATH]
    cordex-migrate apply [PATH] [--dry-run | --write] [--no-todos]

SUBCOMMANDS:
    analyze     Scan Cargo.toml and Rust files, recommend cordex features, list findings
    apply       Preview (default: --dry-run) or apply safe rewrites and TODO markers

FLAGS:
    --dry-run   Compute and print diffs without modifying files (default for `apply`)
    --write     Write safe rewrites and TODO comments in place
    --no-todos  Skip inserting `// TODO(cordex-migrate):` guidance comments
    -h, --help  Show this help message
    -V, --version
                Show version"
    );
}
