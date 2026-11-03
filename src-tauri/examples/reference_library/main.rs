//! Builds the reference library (section 6). Started by
//! `scripts/reference-library.mjs`; see `--help`.
//!
//! The target folder must lie inside the project folder (section 2: scripts
//! never write elsewhere). An existing folder is only replaced with `--force`
//! and only if it is a reference library itself.

mod generate;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

const USAGE: &str = "\
usage: reference_library --out <folder> [--seed <n>] [--documents <n>] [--force]

  --out        target folder inside the project folder
  --seed       seed of all generated data (default 2026)
  --documents  number of documents (default 10000)
  --force      replace an existing reference library in --out";

struct Options {
    out: PathBuf,
    seed: u64,
    documents: usize,
    force: bool,
}

fn parse(arguments: &[String]) -> Result<Options, String> {
    let mut out = None;
    let mut seed = 2026;
    let mut documents = generate::FULL_DOCUMENTS;
    let mut force = false;
    let mut iter = arguments.iter();
    while let Some(argument) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match argument.as_str() {
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--seed" => {
                seed = value("--seed")?
                    .parse()
                    .map_err(|_| "--seed: not a number")?
            }
            "--documents" => {
                documents = value("--documents")?
                    .parse()
                    .map_err(|_| "--documents: not a number")?;
            }
            "--force" => force = true,
            "--help" | "-h" => return Err(USAGE.to_owned()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    let out = out.ok_or_else(|| format!("--out is required\n{USAGE}"))?;
    if documents == 0 {
        return Err("--documents must be at least 1".to_owned());
    }
    Ok(Options {
        out,
        seed,
        documents,
        force,
    })
}

/// The canonical target; refuses anything outside the project folder.
fn checked_target(out: &Path) -> Result<PathBuf, String> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .ok_or("project folder not found")?;
    let target = project_write_lib::security::paths::canonical_prospective(out)
        .ok_or("target folder is not usable")?;
    let project = project_write_lib::security::paths::simplify(&project);
    if !target.starts_with(&project) || target == project {
        return Err(format!(
            "the target must lie inside the project folder {}",
            project.display()
        ));
    }
    Ok(target)
}

/// Removes a previous reference library at `target` (only with `--force` and
/// only if `library.json` carries the reference marker).
fn clear_previous(target: &Path, force: bool) -> Result<(), String> {
    let Ok(mut entries) = std::fs::read_dir(target) else {
        return Ok(());
    };
    if entries.next().is_none() {
        return Ok(());
    }
    if !force {
        return Err(format!(
            "{} is not empty; use --force to replace a reference library",
            target.display()
        ));
    }
    let manifest = std::fs::read(target.join("library.json"))
        .ok()
        .and_then(|data| serde_json::from_slice::<serde_json::Value>(&data).ok());
    let is_reference = manifest
        .as_ref()
        .is_some_and(|m| m.get("referenceLibrary").is_some());
    if !is_reference {
        return Err(format!(
            "{} is not a reference library; it is left unchanged",
            target.display()
        ));
    }
    std::fs::remove_dir_all(target).map_err(|e| format!("cannot remove the old copy: {e}"))
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse(&arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let result = checked_target(&options.out)
        .and_then(|target| clear_previous(&target, options.force).map(|()| target));
    let target = match result {
        Ok(target) => target,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let started = Instant::now();
    let documents = options.documents;
    let outcome = runtime.block_on(async {
        let summary = generate::generate(&target, options.seed, documents, |done| {
            if done % 1000 == 0 || done == documents {
                eprintln!("  {done} / {documents} documents");
            }
        })
        .await?;
        let fingerprint = generate::fingerprint(&target).await?;
        Ok::<_, String>((summary, fingerprint))
    });
    match outcome {
        Ok((summary, fingerprint)) => {
            println!("folder:      {}", target.display());
            println!("seed:        {}", options.seed);
            println!(
                "documents:   {} ({} Markdown, {} text)",
                summary.documents, summary.markdown, summary.text
            );
            println!("content:     {} bytes", summary.bytes);
            println!("time:        {:.1} s", started.elapsed().as_secs_f64());
            println!("fingerprint: {fingerprint}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("reference library failed: {message}");
            ExitCode::FAILURE
        }
    }
}
