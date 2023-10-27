mod pass;

use std::{path, process, str};
use std::fmt::Write;

use rustc_interface::Config;
use rustc_session::config::{self, CheckCfg};
use rustc_span::{FileName, RealFileName, symbol::Symbol, def_id::DefId};
use rustc_errors::registry::Registry;
use rustc_hir::{ItemKind, Node, ExprKind, StmtKind, Ty, TyKind, Expr};
use rustc_middle::ty::{TypeckResults, TyCtxt};
use anyhow::{Result, Context};

use crate::config::Config as LockCheckConfig;
use pass::{AnalysisPass, AnalysisPassTarget};

#[derive(Default)]
struct AnalysisCtx<'tcx> {
    passes: Vec<AnalysisPass<'tcx>>,
}

impl<'tcx> AnalysisCtx<'tcx> {
    fn parse_def_id_from_ty(ty: &Ty, typecheck: &TypeckResults) -> DefId {
        let TyKind::Path(ref ty_path) = ty.kind else {
            invalid_hir();
        };

        typecheck.qpath_res(ty_path, ty.hir_id).def_id()
    }

    fn parse_def_id_from_call_expr(expr: &Expr, typecheck: &TypeckResults) -> DefId {
        let ExprKind::Call(call_expr, _) = expr.kind else {
            invalid_hir();
        };

        let ExprKind::Path(ref ty_path) = call_expr.kind else {
            invalid_hir();
        };

        typecheck.qpath_res(ty_path, call_expr.hir_id).def_id()
    }

    fn parse_passes_from_hir(&mut self, tcx: TyCtxt<'tcx>) {
        let hir = tcx.hir();

        let lock_filler_symbol = Symbol::intern(LOCK_FILLER_FN_NAME);
    
        for id in hir.items() {
            let item = hir.item(id);
            if item.ident.name == lock_filler_symbol {
                let fn_local_def_id = item.owner_id.def_id;
                let typecheck = tcx.typeck(fn_local_def_id);

                // this is the lock filler fn we need to resolve symbol names
                let ItemKind::Fn(_, _, body_id) = item.kind else {
                    invalid_hir();
                };
    
                let fn_body = hir.get(body_id.hir_id);
                let Node::Expr(expr) = fn_body else {
                    invalid_hir();
                };
    
                let ExprKind::Block(block, _) = expr.kind else {
                    invalid_hir();
                };
    
                // each lock rule will generate 3 statements
                for statements in block.stmts.chunks_exact(3) {
                    let StmtKind::Local(lock_new) = statements[0].kind else {
                        invalid_hir();
                    };
    
                    let StmtKind::Local(lock_method) = statements[1].kind else {
                        invalid_hir();
                    };

                    let StmtKind::Local(lock_guard) = statements[2].kind else {
                        invalid_hir();
                    };

                    let lock_def_id = Self::parse_def_id_from_ty(lock_new.ty.unwrap(), &typecheck);
                    let guard_def_id = Self::parse_def_id_from_ty(&lock_guard.ty.unwrap(), &typecheck);

                    let lock_constructor_def_id = Self::parse_def_id_from_call_expr(lock_new.init.unwrap(), &typecheck);
                    let lock_method_def_id = Self::parse_def_id_from_call_expr(lock_method.init.unwrap(), &typecheck);

                    let pass = AnalysisPass::new(AnalysisPassTarget {
                        lock: lock_def_id,
                        lock_constructor: lock_constructor_def_id,
                        lock_method: lock_method_def_id,
                        guard: guard_def_id,
                    }, tcx);
                    self.passes.push(pass);
                }
            }
        }
    }

    fn run_passes(&mut self) {
        for pass in self.passes.iter_mut() {
            pass.run_pass();
        }
    }
}

fn invalid_hir() -> ! {
    panic!("invalid hir data for lock filler resolve function")
}

const LOCK_FILLER_FN_NAME: &'static str = "__lock_check_resolve";

/// This generates a string containing rust code for a function which will call lock type constructor and lock method
/// 
/// This is a hack to get around the fact that I have no idea how to resolve
/// a type name to a DefId except by the lowering process from ast to hir
fn generate_lock_filler(config: &LockCheckConfig) -> Result<String> {
    let mut body = String::new();
    for lock in config.locks.iter() {
        write!(
            body,
            r#"
                let lock: {}<u8> = {}(0);
                // TODO: get rid of unwrap
                let guard_result = {}(&lock);
                let _guard: {}<u8> = guard_result.unwrap();
            "#,
            lock.lock,
            lock.constructor,
            lock.lock_method,
            lock.guard,
        )?;
    }

    Ok(format!(r#"
    #[allow(dead_code)]
    fn {}() {{
        {}
    }}"#, LOCK_FILLER_FN_NAME, body))
}

fn get_rustc_config(lock_check_config: &LockCheckConfig) -> Result<Config> {
    let out = process::Command::new("rustc")
        .arg("--print=sysroot")
        .current_dir(".")
        .output()
        .with_context(|| "could not determine rust sysroot")?;
    let sysroot = str::from_utf8(&out.stdout).expect("invalid utf-8 sysroot path").trim();

    let mut file_data = std::fs::read_to_string(&lock_check_config.crate_root)?;
    let lock_resolve_filler = generate_lock_filler(&lock_check_config)?;
    file_data.push_str(&lock_resolve_filler);

    Ok(Config {
        opts: config::Options {
            maybe_sysroot: Some(path::PathBuf::from(sysroot)),
            ..config::Options::default()
        },
        input: config::Input::Str {
            name: FileName::Real(RealFileName::LocalPath(lock_check_config.crate_root.clone())),
            input: file_data,
        },
        crate_cfg: rustc_hash::FxHashSet::default(),
        crate_check_cfg: CheckCfg::default(),
        output_dir: None,
        output_file: None,
        file_loader: None,
        locale_resources: rustc_driver::DEFAULT_LOCALE_RESOURCES,
        lint_caps: rustc_hash::FxHashMap::default(),
        parse_sess_created: None,
        register_lints: None,
        override_queries: None,
        make_codegen_backend: None,
        registry: Registry::new(&rustc_error_codes::DIAGNOSTICS),
        expanded_args: Vec::new(),
        ice_file: None,
    })
}

pub fn run(config: &LockCheckConfig) -> Result<()> {
    let rustc_config = get_rustc_config(&config)?;

    rustc_interface::run_compiler(rustc_config, |compiler| {
        compiler.enter(|queries| {
            let _crate_ast = queries.parse().unwrap().get_mut().clone();

            queries.global_ctxt().unwrap().enter(|tcx| {
                let mut analysis_ctx = AnalysisCtx::default();

                analysis_ctx.parse_passes_from_hir(tcx);
                analysis_ctx.run_passes();
            });
        });
    });

    Ok(())
}