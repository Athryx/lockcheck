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
mod rustc_config;
mod tyctxt_ext;

use anyhow::Result;

fn run() -> Result<()> {
    let config = config::load_config()?;

    let status = analysis::run(&config)?;
    if status.error_emitted() {
        // cargo panics if we emit an error but don't exit with non zero error code
        std::process::exit(1);
    }

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        println!("{:?}", err);
        std::process::exit(1);
    }
}