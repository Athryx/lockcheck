#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_hash;
extern crate rustc_span;
extern crate rustc_errors;
extern crate rustc_error_codes;
extern crate rustc_error_messages;
extern crate rustc_index;

mod analysis;
mod config;
mod tyctxt_ext;

use std::process::Command;

use anyhow::{Result, bail};

fn run() -> Result<()> {
    let config = config::load_config()?;

    let cargo_build_status = Command::new("cargo")
        .arg("build")
        .status()?;

    if !cargo_build_status.success() {
        bail!("Cargo build failed");
    }

    analysis::run(&config)
}

fn main() {
    if let Err(err) = run() {
        println!("{:?}", err);
        std::process::exit(1);
    }
}