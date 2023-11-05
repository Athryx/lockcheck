use rustc_interface::{interface, Config};
use rustc_session::{EarlyErrorHandler, config::{self, ErrorOutputType}};
use rustc_driver::handle_options;
use rustc_driver::args::arg_expand_all;
use rustc_span::{FileName, RealFileName};
use rustc_errors::registry::Registry;
use anyhow::{Result, bail};

use crate::analysis::generate_lock_filler;
use super::config::Config as LockCheckConfig;

pub fn get_rustc_config(lock_check_config: &LockCheckConfig) -> Result<Config> {
    let mut early_error_handler = EarlyErrorHandler::new(ErrorOutputType::default());

    let full_args = std::env::args().collect::<Vec<_>>();
    // rustc argument functions require first argument is stripped off
    let args = full_args.get(1..).unwrap_or_default();

    let args = arg_expand_all(&early_error_handler, args);
    let Some(matches) = handle_options(&early_error_handler, &args) else {
        bail!("failed to generate rustc config");
    };

    let sopts = config::build_session_options(&mut early_error_handler, &matches);
    let cfg = interface::parse_cfgspecs(&early_error_handler, matches.opt_strs("cfg"));
    let check_cfg = interface::parse_check_cfg(&early_error_handler, matches.opt_strs("check-cfg"));

    let Some(input_file) = matches.free.get(0) else {
        bail!("no input filename given");
    };

    let mut file_data = std::fs::read_to_string(input_file)?;
    let lock_resolve_filler = generate_lock_filler(&lock_check_config)?;
    file_data.push_str(&lock_resolve_filler);

    Ok(Config {
        opts: sopts,
        crate_cfg: cfg,
        crate_check_cfg: check_cfg,
        input: config::Input::Str {
            name: FileName::Real(RealFileName::LocalPath(input_file.into())),
            input: file_data,
        },
        output_file: None,
        output_dir: None,
        ice_file: None,
        file_loader: None,
        locale_resources: rustc_driver::DEFAULT_LOCALE_RESOURCES,
        lint_caps: rustc_hash::FxHashMap::default(),
        parse_sess_created: None,
        register_lints: None,
        override_queries: None,
        make_codegen_backend: None,
        registry: Registry::new(&rustc_error_codes::DIAGNOSTICS),
        expanded_args: args,
    })
}
