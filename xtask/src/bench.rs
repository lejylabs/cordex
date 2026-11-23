//! `cargo xtask bench`: run the benchmarks (PLAN §16.3) and compare them
//! with `benches/baseline.json`.
//!
//! - criterion benches: `cordex-model` `parse`, `cordex-cache` `cache`; the
//!   mean of each benchmark is read from `target/criterion`.
//! - allocation bench: `cordex-model` `alloc` (dhat), printed as is.
//!
//! A benchmark more than 10 % slower than the baseline is reported. Local
//! numbers only compare runs on the same machine; official numbers come
//! from the production KVM. `--save` writes the current means as the new
//! baseline. Extra arguments after `--` go to criterion (e.g.
//! `-- --measurement-time 3`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{read, run, workspace_root};

const BASELINE: &str = "benches/baseline.json";
/// Report a benchmark that is this much slower than the baseline.
const REGRESSION: f64 = 1.10;

const CRITERION_BENCHES: &[(&str, &str)] = &[("cordex-model", "parse"), ("cordex-cache", "cache")];

pub(crate) fn bench(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    let save = args.iter().any(|a| a == "--save");
    let passthrough: Vec<&str> = args
        .iter()
        .skip_while(|a| *a != "--")
        .skip(1)
        .map(String::as_str)
        .collect();

    for (package, bench) in CRITERION_BENCHES {
        let mut cmd = vec!["bench", "-p", package, "--bench", bench, "--locked", "--"];
        cmd.extend(&passthrough);
        run(&cmd, &[])?;
    }
    run(
        &[
            "bench",
            "-p",
            "cordex-model",
            "--bench",
            "alloc",
            "--locked",
        ],
        &[],
    )?;

    let current = collect(&root.join("target").join("criterion"));
    if current.is_empty() {
        return Err("no criterion results found in target/criterion".to_owned());
    }
    let baseline_path = root.join(BASELINE);
    let baseline = read(&baseline_path)
        .map(|t| parse_baseline(&t))
        .unwrap_or_default();

    println!(
        "\n{:<55} {:>12} {:>12} {:>8}",
        "benchmark", "mean", "baseline", "change"
    );
    let mut regressions = 0;
    for (name, ns) in &current {
        let (base, change) = match baseline.get(name) {
            Some(base) => {
                let ratio = ns / base;
                if ratio > REGRESSION {
                    regressions += 1;
                }
                (fmt_ns(*base), format!("{:+.1} %", (ratio - 1.0) * 100.0))
            }
            None => ("-".to_owned(), "new".to_owned()),
        };
        println!("{name:<55} {:>12} {base:>12} {change:>8}", fmt_ns(*ns));
    }

    if save {
        fs::create_dir_all(root.join("benches")).map_err(|e| e.to_string())?;
        fs::write(&baseline_path, render_baseline(&current))
            .map_err(|e| format!("{}: {e}", baseline_path.display()))?;
        println!("\nwrote {BASELINE}");
    } else if regressions > 0 {
        println!(
            "\n{regressions} benchmark(s) more than {:.0} % slower than {BASELINE}; \
             run twice before trusting it (machines vary)",
            (REGRESSION - 1.0) * 100.0
        );
    }
    Ok(())
}

/// `group/name` → mean in nanoseconds, from criterion's `new/estimates.json`.
fn collect(dir: &Path) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    let Ok(groups) = fs::read_dir(dir) else {
        return out;
    };
    for group in groups.flatten() {
        let Ok(benches) = fs::read_dir(group.path()) else {
            continue;
        };
        for bench in benches.flatten() {
            let estimates = bench.path().join("new").join("estimates.json");
            let Ok(text) = fs::read_to_string(&estimates) else {
                continue;
            };
            if let Some(mean) = mean_ns(&text) {
                let name = format!(
                    "{}/{}",
                    group.file_name().to_string_lossy(),
                    bench.file_name().to_string_lossy()
                );
                out.insert(name, mean);
            }
        }
    }
    out
}

/// `"mean":{…,"point_estimate":123.4,…}` without a JSON dependency.
fn mean_ns(estimates: &str) -> Option<f64> {
    let mean = &estimates[estimates.find("\"mean\"")?..];
    let value = &mean[mean.find("\"point_estimate\":")? + 17..];
    let end = value.find([',', '}'])?;
    value[..end].trim().parse().ok()
}

fn parse_baseline(text: &str) -> BTreeMap<String, f64> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.trim().trim_end_matches(',').split_once(':')?;
            let key = key.trim().trim_matches('"');
            let value = value.trim().parse().ok()?;
            (!key.is_empty()).then(|| (key.to_owned(), value))
        })
        .collect()
}

fn render_baseline(values: &BTreeMap<String, f64>) -> String {
    let body: Vec<String> = values
        .iter()
        .map(|(k, v)| format!("  \"{k}\": {v:.1}"))
        .collect();
    format!("{{\n{}\n}}\n", body.join(",\n"))
}

fn fmt_ns(ns: f64) -> String {
    if ns >= 1_000_000.0 {
        format!("{:.2} ms", ns / 1_000_000.0)
    } else if ns >= 1_000.0 {
        format!("{:.2} µs", ns / 1_000.0)
    } else {
        format!("{ns:.1} ns")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_criterion_estimates_and_baselines() {
        let estimates = r#"{"mean":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":2.0},"point_estimate":1415.3,"standard_error":1.0},"median":{"point_estimate":9.0}}"#;
        assert_eq!(mean_ns(estimates), Some(1415.3));
        let mut map = BTreeMap::new();
        map.insert("a/b".to_owned(), 12.5);
        map.insert("c/d".to_owned(), 1_000.0);
        assert_eq!(parse_baseline(&render_baseline(&map)), map);
        assert_eq!(fmt_ns(2_500_000.0), "2.50 ms");
    }
}
