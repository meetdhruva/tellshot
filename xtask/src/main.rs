//! Repository automation. Written in Rust so it runs identically on Windows, macOS and Linux.
//!
//! ```text
//! cargo xtask ci        # everything CI runs, in the same order; run before pushing
//! cargo xtask fmt       # format the workspace
//! cargo xtask bless     # regenerate golden images after an intended rendering change
//! cargo xtask schema    # regenerate docs/schema/*.json from the CLI
//! ```

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "ci" => ci(),
        "fmt" => run("cargo", &["fmt", "--all"]),
        "bless" => bless(),
        "schema" => schema(),
        _ => {
            let _ignored = writeln!(
                std::io::stderr(),
                "usage: cargo xtask <ci|fmt|bless|schema>\n\n\
                 ci      run every check CI runs (fmt, clippy, tests, docs, deny, typos, machete)\n\
                 fmt     format the workspace\n\
                 bless   regenerate golden images after an intended rendering change\n\
                 schema  regenerate docs/schema/*.json"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ignored = writeln!(std::io::stderr(), "xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn ci() -> Result<(), String> {
    step("format", || run("cargo", &["fmt", "--all", "--check"]))?;
    step("clippy", || {
        run(
            "cargo",
            &["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"],
        )
    })?;
    step("tests", || {
        if tool_exists(&["cargo", "nextest"]) {
            run("cargo", &["nextest", "run", "--workspace", "--locked", "--profile", "ci"])?;
            // nextest does not run doctests.
            run("cargo", &["test", "--workspace", "--doc", "--locked"])
        } else {
            run("cargo", &["test", "--workspace", "--locked"])
        }
    })?;
    step("docs", || {
        run_env(
            "cargo",
            &["doc", "--workspace", "--no-deps", "--document-private-items", "--locked"],
            &[("RUSTDOCFLAGS", "-D warnings")],
        )
    })?;
    optional_step(&["cargo", "deny"], "licences, advisories, bans", || {
        run("cargo", &["deny", "--locked", "check"])
    })?;
    optional_step(&["typos"], "spelling", || run("typos", &[]))?;
    optional_step(&["cargo", "machete"], "unused dependencies", || run("cargo", &["machete"]))?;
    say("all checks passed");
    Ok(())
}

fn bless() -> Result<(), String> {
    run_env(
        "cargo",
        &["test", "--package", "tellshot-render", "--test", "golden"],
        &[("TELLSHOT_BLESS", "1")],
    )
}

fn schema() -> Result<(), String> {
    let dir = workspace_root().join("docs").join("schema");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for which in ["edit", "document"] {
        let out = Command::new("cargo")
            .args(["run", "--quiet", "--package", "tellshot-cli", "--", "schema", which])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("schema {which} failed: {}", String::from_utf8_lossy(&out.stderr)));
        }
        let path = dir.join(format!("{which}.schema.json"));
        #[expect(
            clippy::disallowed_methods,
            reason = "generated file in the repository, not user data"
        )]
        std::fs::write(&path, &out.stdout).map_err(|e| e.to_string())?;
        say(&format!("wrote {}", path.display()));
    }
    Ok(())
}

fn step(name: &str, f: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    say(&format!("── {name}"));
    f().map_err(|e| format!("{name} failed: {e}"))
}

fn optional_step(
    tool: &[&str],
    what: &str,
    f: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if tool_exists(tool) {
        step(what, f)
    } else {
        say(&format!("── {what}: skipped, `{}` is not installed (CI runs it)", tool.join(" ")));
        Ok(())
    }
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    run_env(program, args, &[])
}

fn run_env(program: &str, args: &[&str], env: &[(&str, &str)]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .current_dir(workspace_root())
        .status()
        .map_err(|e| format!("could not start {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{program} {}` exited with {status}", args.join(" ")))
    }
}

/// True when `program [subcommand] --help` succeeds, for example `["cargo", "deny"]`.
fn tool_exists(command: &[&str]) -> bool {
    let Some((program, rest)) = command.split_first() else {
        return false;
    };
    Command::new(program).args(rest).arg("--version").output().is_ok_and(|o| o.status.success())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn say(msg: &str) {
    let _ignored = writeln!(std::io::stdout().lock(), "{msg}");
}
