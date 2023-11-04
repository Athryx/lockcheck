use std::process;
use std::sync::Arc;

use cargo::{Config, CliResult, CargoResult, ops};
use cargo::util::command_prelude::*;
use cargo::core::{Shell, PackageId, Target, compiler::Executor};
use cargo_util::ProcessBuilder;
use anyhow::{Result, bail};

struct LockCheckExecutor;

impl Executor for LockCheckExecutor {
    fn exec(
        &self,
        cmd: &ProcessBuilder,
        _id: PackageId,
        _target: &Target,
        _mode: CompileMode,
        on_stdout_line: &mut dyn FnMut(&str) -> CargoResult<()>,
        on_stderr_line: &mut dyn FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()> {
        // get mutable access to command and change command to run lockcheck
        let mut cmd = cmd.clone();
        cmd.program("lockcheck");

        cmd.exec_with_streaming(on_stdout_line, on_stderr_line, false)
            .map(drop)
    }
}

/// This uses cargo to run lockcheck on all crates in the package
fn run(config: &mut Config) -> CliResult {
    let args = Command::new("cargo_lockcheck")
        .subcommand(subcommand("lockcheck"))
        .get_matches();

    config.configure(
        0,
        false,
        Some("always"),
        true, // stops cargo from determining if lock file is out of date
        true, // stops cargo.lock from being modified
        true, // cargo will not access the network
        &None, // default target dir
        &[],
        &[],
    )?;

    let workspace = args.workspace(config)?;
    let mut compile_opts = args.compile_options(
        config,
        CompileMode::Build,
        Some(&workspace),
        ProfileChecking::Custom,
    )?;
    // forces cargo to run lock check
    compile_opts.build_config.force_rebuild = true;

    let executor: Arc<dyn Executor> = Arc::new(LockCheckExecutor);

    ops::compile_with_exec(
        &workspace,
        &compile_opts,
        &executor,
    )?;

    Ok(())
}

/// Runs cargo build
/// 
/// This is needed because lockcheck needs the mir of dependancies to be generated
fn run_cargo_build() -> Result<()> {
    let cargo_build_status = process::Command::new("cargo")
        .arg("build")
        .status()?;

    if !cargo_build_status.success() {
        bail!("Cargo build failed");
    }

    Ok(())
}

fn main() {
    if let Err(err) = run_cargo_build() {
        println!("{:?}", err);
        process::exit(1);
    }

    let mut config = match Config::default() {
        Ok(config) => config,
        Err(err) => {
            let mut shell = Shell::new();
            cargo::exit_with_error(err.into(), &mut shell);
        }
    };

    if let Err(err) = run(&mut config) {
        cargo::exit_with_error(err, &mut config.shell());
    }
}
