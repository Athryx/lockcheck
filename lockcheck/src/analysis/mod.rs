mod pass;

use std::str;
use std::fmt::Write;
use std::rc::Rc;
use std::ops::BitOr;

use rustc_session::Session;
use rustc_span::{symbol::Symbol, def_id::DefId};
use rustc_hir::{ItemKind, Node, ExprKind, StmtKind, Ty, TyKind, Expr};
use rustc_middle::ty::{TypeckResults, TyCtxt};
use anyhow::Result;

use crate::config::Config as LockCheckConfig;
use crate::rustc_config::get_rustc_config;
use pass::{AnalysisPass, AnalysisPassTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorStatus {
    Ok,
    DeadlockDetected,
}

impl ErrorStatus {
    pub fn error_emitted(self) -> bool {
        matches!(self, Self::DeadlockDetected)
    }
}

impl BitOr for ErrorStatus {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Ok, Self::Ok) => Self::Ok,
            _ => Self::DeadlockDetected,
        }
    }
}

#[derive(Default)]
struct AnalysisCtx<'tcx> {
    passes: Vec<AnalysisPass<'tcx>>,
}

impl<'tcx> AnalysisCtx<'tcx> {
    fn parse_passes_from_hir(tcx: TyCtxt<'tcx>, session: &Rc<Session>) -> Self {
        let mut passes = Vec::new();

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
                    }, tcx, session.clone());
                    passes.push(pass);
                }
            }
        }

        AnalysisCtx {
            passes,
        }
    }

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

    fn run_passes(&mut self) -> ErrorStatus {
        let mut status = ErrorStatus::Ok;

        for pass in self.passes.iter_mut() {
            status = status | pass.run_pass();
        }

        status
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
pub fn generate_lock_filler(config: &LockCheckConfig) -> Result<String> {
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

pub fn run(config: &LockCheckConfig) -> Result<ErrorStatus> {
    let rustc_config = get_rustc_config(&config)?;

    let status = rustc_interface::run_compiler(rustc_config, |compiler| {
        compiler.enter(|queries| {
            let _crate_ast = queries.parse().unwrap().get_mut().clone();

            queries.global_ctxt().unwrap().enter(|tcx| {
                let mut analysis_ctx = AnalysisCtx::parse_passes_from_hir(tcx, compiler.session());

                analysis_ctx.run_passes()
            })
        })
    });

    Ok(status)
}