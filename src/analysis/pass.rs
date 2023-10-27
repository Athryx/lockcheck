use std::collections::{HashSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};

use rustc_span::{symbol::Symbol, def_id::DefId};
use rustc_middle::ty::{TyCtxt, TyKind, Ty};
use rustc_middle::mir::{START_BLOCK, BasicBlock, Terminator, TerminatorKind, Operand, Const, ConstValue, Body};
use rustc_hir::ItemKind;

use super::LOCK_FILLER_FN_NAME;

#[derive(Debug)]
pub struct AnalysisPassTarget {
    pub lock: DefId,
    pub lock_constructor: DefId,
    pub lock_method: DefId,
    pub guard: DefId,
}

static NEXT_LOCK_CLASS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LockClass(u64);

impl LockClass {
    fn new() -> Self {
        LockClass(NEXT_LOCK_CLASS.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug)]
pub struct LockInvocation {
    class: LockClass,
    dependant_classes: HashSet<LockClass>,
}

pub struct AnalysisPass<'tcx> {
    tcx: TyCtxt<'tcx>,
    pass_target: AnalysisPassTarget,
    invocations: HashMap<BasicBlock, LockInvocation>,
    lock_class_ty_map: HashMap<Ty<'tcx>, LockClass>,
}

impl<'tcx> AnalysisPass<'tcx> {
    pub fn new(pass_target: AnalysisPassTarget, tcx: TyCtxt<'tcx>) -> Self {
        AnalysisPass {
            tcx,
            pass_target,
            invocations: HashMap::new(),
            lock_class_ty_map: HashMap::new(),
        }
    }

    fn get_lock_cass_for_type(&mut self, ty: Ty<'tcx>) -> LockClass {
        *self.lock_class_ty_map.entry(ty).or_insert_with(|| LockClass::new())
    }

    fn is_terminator_lock_invocation(&self, terminator: &Terminator) -> bool {
        let TerminatorKind::Call { func, .. } = &terminator.kind else {
            return false;
        };

        let Operand::Constant(c) = func else {
            return false;
        };

        let Const::Val(ConstValue::ZeroSized, fn_type) = c.const_ else {
            return false;
        };

        let TyKind::FnDef(def_id, _) = fn_type.kind() else {
            return false;
        };

        *def_id == self.pass_target.lock_method
    }

    fn lock_class_from_terminator(&mut self, mir_body: &Body<'tcx>, basic_block: BasicBlock) -> Option<LockClass> {
        let terminator = (&mir_body.basic_blocks[basic_block]).terminator.as_ref()?;

        if !self.is_terminator_lock_invocation(terminator) {
            return None;
        }

        let TerminatorKind::Call { args, .. } = &terminator.kind else {
            return None;
        };

        // Find the first argument which is a Mutex, and use that mutex types generic arg to get the lock class
        for arg in args.iter() {
            let arg_type = arg.ty(&mir_body.local_decls, self.tcx).peel_refs();
            let TyKind::Adt(adt_def, generic_args) = arg_type.kind() else {
                continue;
            };

            if adt_def.did() == self.pass_target.lock {
                if generic_args.len() != 1 {
                    // FIXME: don't panic here
                    panic!("lockcheck only works on mutexes with 1 generic argument");
                }

                // FIXME: don't panic here
                let generic_type = generic_args[0].expect_ty();
                return Some(self.get_lock_cass_for_type(generic_type));
            }
        }

        None
    }

    pub fn run_pass(&mut self) {
        let hir = self.tcx.hir();

        let lock_filler_symbol = Symbol::intern(LOCK_FILLER_FN_NAME);

        for id in hir.items() {
            let item = hir.item(id);

            // only functions have mir data to analyse
            if !matches!(item.kind, ItemKind::Fn(..)) {
                continue;
            }

            // ignore lock filler symbol inserted by lockcheck
            if item.ident.name == lock_filler_symbol {
                continue;
            }

            let mir = self.tcx.optimized_mir(item.owner_id.to_def_id());
            let start_block = &mir.basic_blocks[START_BLOCK];
            let _tmp = self.lock_class_from_terminator(mir, START_BLOCK);
            println!("{_tmp:?}");
            let TerminatorKind::Call { ref func, .. } = start_block.terminator.as_ref().unwrap().kind else {
                panic!("wo");
            };
            let Operand::Constant(c) = func else {
                panic!("a");
            };
            println!("{:#?}", c.const_);

            return;
            println!("{mir:#?}");
        }
    }
}