use std::collections::{HashSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::cell::RefCell;

use rustc_span::{symbol::Symbol, def_id::DefId};
use rustc_middle::ty::{TyCtxt, TyKind, Ty};
use rustc_middle::mir::{BasicBlock, Terminator, TerminatorKind, Operand, Const, ConstValue, Body, Local, Statement, StatementKind, Rvalue};
use rustc_middle::mir::traversal::reachable;
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
    dependant_classes: RefCell<HashSet<LockClass>>,
}

impl LockInvocation {
    fn new(class: LockClass) -> Self {
        LockInvocation {
            class,
            dependant_classes: RefCell::new(HashSet::new()),
        }
    }
}

/// Basic Block ID
/// 
/// Uniquely identifies any basic block in the whole program
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Bbid {
    def_id: DefId,
    basic_block: BasicBlock,
}

pub struct AnalysisPass<'tcx> {
    tcx: TyCtxt<'tcx>,
    pass_target: AnalysisPassTarget,
    invocations: HashMap<Bbid, LockInvocation>,
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
        let terminator = mir_body.basic_blocks[basic_block].terminator();

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

    fn collect_invocations_for_body(&mut self, def_id: DefId, mir_body: &Body<'tcx>) {
        for (basic_block, _) in reachable(mir_body) {
            if let Some(lock_class) = self.lock_class_from_terminator(mir_body, basic_block) {
                let bbid = Bbid {
                    def_id,
                    basic_block,
                };

                self.invocations.insert(bbid, LockInvocation::new(lock_class));
            }
        }
    }

    fn collect_invocations(&mut self) {
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

            let def_id = item.owner_id.to_def_id();
            let mir = self.tcx.optimized_mir(item.owner_id.to_def_id());

            self.collect_invocations_for_body(def_id, mir);
        }
    }

    /// Analyses collected lock invocations and determines their dependant lock classes
    fn collect_dependant_lock_classes(&mut self) {
        for (bbid, invocation) in self.invocations.iter() {
            let mir_body = self.tcx.optimized_mir(bbid.def_id);

            let basic_block_data = &mir_body[bbid.basic_block];
            let TerminatorKind::Call { target: Some(target), destination, .. } = basic_block_data.terminator().kind else {
                panic!("lock invocation is expected to be call");
            };

            let collector = DependantClassCollector {
                invocation_map: &self.invocations,
                mir_body_def_id: bbid.def_id,
                mir_body,
                dependant_classes: HashSet::new(),
            };
            let dependant_classes = collector.collect(target, destination.local);
            *invocation.dependant_classes.borrow_mut() = dependant_classes;
        }
    }

    pub fn run_pass(&mut self) {
        self.collect_invocations();
        self.collect_dependant_lock_classes();

        println!("{:#?}", self.invocations);
    }
}

struct DependantClassCollector<'a, 'tcx> {
    invocation_map: &'a HashMap<Bbid, LockInvocation>,
    mir_body_def_id: DefId,
    mir_body: &'tcx Body<'tcx>,
    dependant_classes: HashSet<LockClass>,
}

impl DependantClassCollector<'_, '_> {
    fn collect(mut self, basic_block: BasicBlock, lock_local: Local) -> HashSet<LockClass> {
        self.collect_dependant_classes_inner(basic_block, lock_local);

        let Self { dependant_classes, .. } = self;
        dependant_classes
    }

    fn collect_dependant_classes_inner(&mut self, mut basic_block: BasicBlock, mut current_local: Local) {
        loop {
            // mark dependant class if this current block also is a lock invocation
            let current_bbid = Bbid {
                def_id: self.mir_body_def_id,
                basic_block,
            };
            if let Some(invocation) = self.invocation_map.get(&current_bbid) {
                self.dependant_classes.insert(invocation.class);
            }

            let basic_block_data = &self.mir_body[basic_block];

            for statement in basic_block_data.statements.iter() {
                current_local = calculate_new_local_after_statement(statement, current_local);
            }

            match &basic_block_data.terminator().kind {
                TerminatorKind::Goto { target } => basic_block = *target,
                TerminatorKind::SwitchInt { targets, .. } => {
                    for (_, target) in targets.iter() {
                        self.collect_dependant_classes_inner(target, current_local);
                    }
                    return;
                },
                TerminatorKind::UnwindResume => return,
                TerminatorKind::UnwindTerminate(_) => return,
                TerminatorKind::Return => return,
                TerminatorKind::Unreachable => panic!("found unreachable block"),
                TerminatorKind::Drop { place, target, .. } => {
                    if place.local == current_local {
                        return;
                    } else {
                        basic_block = *target;
                    }
                },
                TerminatorKind::Call { .. } => todo!(),
                TerminatorKind::Assert { target, .. } => basic_block = *target,
                TerminatorKind::Yield { .. } => todo!(),
                // aparently this is like a return from generator?
                TerminatorKind::GeneratorDrop => return,
                TerminatorKind::FalseEdge { real_target, .. } => basic_block = *real_target,
                TerminatorKind::FalseUnwind { real_target, .. } => basic_block = *real_target,
                // TODO: detect if inline asm operands is local we are using
                TerminatorKind::InlineAsm { destination, .. } => {
                    if let Some(dest) = destination {
                        basic_block = *dest;
                    } else {
                        // inline asm is diverging
                        return;
                    }
                }
            }
        }
    }
}

/// Tracks where the given local will be after executing the statement
///
/// Used to track which local the lock guard is in
/// This is currently a flawed implenentation which does not consider projections
fn calculate_new_local_after_statement(statement: &Statement, current_local: Local) -> Local {
    match &statement.kind {
        StatementKind::Assign(assign_data) => {
            let from_operand = match &assign_data.1 {
                Rvalue::Use(operand) => operand,
                // FIXME: handle this case correctly
                // aggregute is used when constructing a struct or enum, so the mutex guard could be put in a struct
                Rvalue::Aggregate(_, _) => return current_local,
                // the rest of rvalues for the most part won't be used on something like a lock guard
                _ => return current_local,
            };

            match from_operand {
                // FIXME: I don't know if this is actually true, I think after drop elaboration
                // the compiler may turn moves into copies
                Operand::Copy(place) if place.local == current_local => panic!("lock guard cannot be copied"),
                Operand::Move(place) if place.local == current_local => place.local,
                _ => current_local,
            }
        },
        StatementKind::Deinit(place) if place.local == current_local => panic!("invalid deinit"),
        // calling storage live on an already alive local is ub
        StatementKind::StorageLive(local) if *local == current_local => panic!("invalid storage live"),
        StatementKind::StorageDead(local) if *local == current_local => panic!("invalid storage dead"),
        // any other statement assume it doesn't do anything
        _ => current_local,
    }
}