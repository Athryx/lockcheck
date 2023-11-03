use std::collections::{HashSet, HashMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::cell::RefCell;
use std::rc::Rc;

use rustc_session::Session;
use rustc_span::{Span, symbol::Symbol, def_id::DefId};
use rustc_middle::ty::{TyCtxt, TyKind, Ty};
use rustc_middle::mir::{BasicBlock, Terminator, TerminatorKind, Operand, Const, ConstValue, Body, Local, Statement, StatementKind, Rvalue, START_BLOCK};
use rustc_middle::mir::traversal::reachable;
use rustc_hir::ItemKind;
use rustc_error_messages::MultiSpan;

use super::LOCK_FILLER_FN_NAME;
use crate::diagnostic::deadlock_error;

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

#[derive(Default)]
struct LockClassTyMap<'tcx> {
    class_to_ty: HashMap<LockClass, Ty<'tcx>>,
    ty_to_class: HashMap<Ty<'tcx>, LockClass>,
}

impl<'tcx> LockClassTyMap<'tcx> {
    fn get_lock_class(&mut self, ty: Ty<'tcx>) -> LockClass {
        if let Some(class) = self.ty_to_class.get(&ty) {
            *class
        } else {
            let class = LockClass::new();
            self.class_to_ty.insert(class, ty);
            self.ty_to_class.insert(ty, class);
            class
        }
    }

    fn get_ty(&self, class: LockClass) -> Ty<'tcx> {
        self.class_to_ty[&class]
    }
}

#[derive(Debug)]
pub struct LockInvocation {
    class: LockClass,
    child_invocations: RefCell<HashSet<Bbid>>,
    span: Span,
}

impl LockInvocation {
    fn new(class: LockClass, span: Span,) -> Self {
        LockInvocation {
            class,
            child_invocations: RefCell::new(HashSet::new()),
            span,
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

impl Bbid {
    /// Returns the bbid of the basic block at the start of a given function
    fn fn_start(def_id: DefId) -> Self {
        Bbid {
            def_id,
            basic_block: START_BLOCK,
        }
    }

    /// Creates a new basic block id in the same function as the current one with a differnt basic block
    fn with_basic_block(&self, basic_block: BasicBlock) -> Self {
        Bbid {
            def_id: self.def_id,
            basic_block,
        }
    }
}

pub struct AnalysisPass<'tcx> {
    tcx: TyCtxt<'tcx>,
    session: Rc<Session>,
    pass_target: AnalysisPassTarget,
    invocations: HashMap<Bbid, LockInvocation>,
    lock_class_ty_map: LockClassTyMap<'tcx>,
    // this ensures errors are emitted in order
    errors: RefCell<BTreeSet<DeadlockError<'tcx>>>,
}

impl<'tcx> AnalysisPass<'tcx> {
    pub fn new(pass_target: AnalysisPassTarget, tcx: TyCtxt<'tcx>, session: Rc<Session>) -> Self {
        AnalysisPass {
            tcx,
            session,
            pass_target,
            invocations: HashMap::new(),
            lock_class_ty_map: LockClassTyMap::default(),
            errors: RefCell::default(),
        }
    }

    fn is_terminator_lock_invocation(&self, terminator: &Terminator) -> bool {
        if let Some(def_id) = get_fn_def_id_from_terminator(terminator) {
            def_id == self.pass_target.lock_method
        } else {
            false
        }
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
                return Some(self.lock_class_ty_map.get_lock_class(generic_type));
            }
        }

        None
    }

    fn collect_invocations_for_body(&mut self, def_id: DefId, mir_body: &Body<'tcx>) {
        for (basic_block, _) in reachable(mir_body) {
            let terminator = mir_body.basic_blocks[basic_block].terminator();
            if let Some(lock_class) = self.lock_class_from_terminator(mir_body, basic_block) {
                let bbid = Bbid {
                    def_id,
                    basic_block,
                };

                self.invocations.insert(bbid, LockInvocation::new(lock_class, terminator.source_info.span));
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
                tcx: self.tcx,
                invocation_map: &self.invocations,
                dependant_classes: HashSet::new(),
                visited_blocks: HashSet::new(),
            };
            let child_invocations = collector.collect(bbid.with_basic_block(target), destination.local);
            *invocation.child_invocations.borrow_mut() = child_invocations;
        }
    }

    /// Creates a map for each lock class to which lock classes are called while the current lock class is locked
    fn get_dependant_map(&self) -> HashMap<LockClass, HashSet<LockClass>> {
        let mut dependant_map = HashMap::new();

        for invocation in self.invocations.values() {
            let current_invocation_dependancies: &mut HashSet<LockClass> = dependant_map
                .entry(invocation.class)
                .or_default();

            for child_id in invocation.child_invocations.borrow().iter() {
                let child_invocation = &self.invocations[child_id];
                current_invocation_dependancies.insert(child_invocation.class);
            }
        }

        dependant_map
    }

    pub fn run_pass(&mut self) {
        self.collect_invocations();
        self.collect_dependant_lock_classes();
        println!("{:#?}", self.invocations);

        let dependant_map = self.get_dependant_map();
        println!("{dependant_map:#?}");

        for invocation in self.invocations.values() {
            for child_id in invocation.child_invocations.borrow().iter() {
                let child_invocation = &self.invocations[child_id];
                let child_dependancies = &dependant_map[&child_invocation.class];

                // if somewhere else our lock class is locked after the child, it is a deadlock potential error
                if child_dependancies.contains(&invocation.class) {
                    self.emit_deadlock_error(invocation, child_invocation);
                }
            }
        }

        self.emit_all_errors();
    }

    fn emit_deadlock_error(&self, parent_invocation: &LockInvocation, child_invocation: &LockInvocation) {
        let parent_ty = self.lock_class_ty_map.get_ty(parent_invocation.class);
        let child_ty = self.lock_class_ty_map.get_ty(child_invocation.class);

        let error = DeadlockError {
            parent_ty,
            child_ty,
            parent_span: parent_invocation.span,
            child_span: child_invocation.span,
        };

        self.errors.borrow_mut().insert(error);
    }

    fn emit_all_errors(&self) {
        for error in self.errors.borrow().iter() {
            let mut multi_span = MultiSpan::from_span(error.child_span);
            multi_span.push_span_label(error.parent_span, format!("lock class `{}` first locked here", error.parent_ty));
            multi_span.push_span_label(error.child_span, format!("deadlock occurs when lock class `{}` locked here", error.child_ty));
        
            self.session.struct_span_err(multi_span, "potential deadlock detected").emit();
        }
    }
}

struct DeadlockError<'tcx> {
    parent_ty: Ty<'tcx>,
    child_ty: Ty<'tcx>,
    parent_span: Span,
    child_span: Span,
}

impl PartialEq for DeadlockError<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.child_span == other.child_span
    }
}

impl Eq for DeadlockError<'_> {}

impl PartialOrd for DeadlockError<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DeadlockError<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.child_span.cmp(&other.child_span)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LocalBlockPair {
    block: Bbid,
    local: Local,
}

/// Indicates what happed to a lock guard passed in a function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuardState {
    Returned,
    Dropped,
    Undetermined,
}

impl GuardState {
    fn combine(self, other: GuardState) -> Self {
        match (self, other) {
            (Self::Returned, _) | (_, Self::Returned) => Self::Returned,
            (Self::Dropped, _) | (_, Self::Dropped) => Self::Dropped,
            _ => Self::Undetermined,
        }
    }
}

struct DependantClassCollector<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    invocation_map: &'a HashMap<Bbid, LockInvocation>,
    dependant_classes: HashSet<Bbid>,
    visited_blocks: HashSet<LocalBlockPair>,
}

impl DependantClassCollector<'_, '_> {
    fn collect(mut self, basic_block_id: Bbid, lock_local: Local) -> HashSet<Bbid> {
        self.collect_inner(basic_block_id, lock_local);

        let Self { dependant_classes, .. } = self;
        dependant_classes
    }

    fn collect_inner(&mut self, basic_block_id: Bbid, mut current_local: Local) -> GuardState {
        let mut basic_block = basic_block_id.basic_block;
        let mut guard_state = GuardState::Undetermined;
        let mir_body = self.tcx.optimized_mir(basic_block_id.def_id);

        loop {
            let current_bbid = basic_block_id.with_basic_block(basic_block);

            // don't visit a block for which we already examined the flow for the given local
            let local_block_pair = LocalBlockPair {
                block: current_bbid,
                local: current_local,
            };
            if self.visited_blocks.contains(&local_block_pair) {
                return GuardState::Undetermined;
            }
            self.visited_blocks.insert(local_block_pair);

            // mark dependant class if this current block also is a lock invocation
            if self.invocation_map.contains_key(&current_bbid) {
                self.dependant_classes.insert(current_bbid);
            }

            let basic_block_data = &mir_body[basic_block];

            for statement in basic_block_data.statements.iter() {
                current_local = calculate_new_local_after_statement(statement, current_local);
            }

            match &basic_block_data.terminator().kind {
                TerminatorKind::Goto { target } => basic_block = *target,
                TerminatorKind::SwitchInt { targets, .. } => {
                    for (_, target) in targets.iter() {
                        // this runs for each branch except the otherwise
                        guard_state = guard_state.combine(self.collect_inner(basic_block_id.with_basic_block(target), current_local));
                    }

                    // now we run for the otherwise branch
                    basic_block = targets.otherwise();
                },
                TerminatorKind::UnwindResume => return guard_state.combine(GuardState::Undetermined),
                TerminatorKind::UnwindTerminate(_) => return guard_state.combine(GuardState::Undetermined),
                TerminatorKind::Return => {
                    // if current local is the return place
                    if current_local == Local::from_u32(0) {
                        return guard_state.combine(GuardState::Returned);
                    } else {
                        panic!("function returned while guard not dropped");
                    }
                }
                TerminatorKind::Unreachable => return guard_state.combine(GuardState::Undetermined),
                TerminatorKind::Drop { place, target, .. } => {
                    if place.local == current_local {
                        return guard_state.combine(GuardState::Dropped);
                    } else {
                        basic_block = *target;
                    }
                },
                TerminatorKind::Call { args, destination, target, .. } => {
                    if destination.local == current_local {
                        panic!("lock guard overwritten while not dropped");
                    }

                    // If the guard is passed into the function, this will be the local of the guard
                    let mut guard_arg_local = None;
                    for (i, arg) in args.iter().enumerate() {
                        // FIXME: we should examine function that is called to see if it potantially
                        // stores mutext guard somewhere or returns the mutex guard again
                        // currently we assume the function just drops it
                        
                        match arg {
                            // lock guard is moved into the function and assumed for now to be dropped in that function, finish analysis
                            Operand::Move(place) if place.local == current_local => {
                                guard_arg_local = Some(Local::from_u32(i as u32 + 1));
                                break;
                            },
                            // FIXME: I don't know if this is actually true, I think after drop elaboration
                            // the compiler may turn moves into copies
                            Operand::Copy(place) if place.local == current_local => panic!("lock guard cannot be copied"),
                            _ => continue,
                        }
                    }

                    let fn_def_id = get_fn_def_id_from_terminator(&basic_block_data.terminator());
                    match (guard_arg_local, fn_def_id) {
                        // if lock guard was passed into function, but we don't know which function, just assume it was dropped
                        // FIXME: this might not be correct
                        (Some(_arg), None) => return guard_state.combine(GuardState::Dropped),
                        (Some(arg), Some(fn_def_id)) => {
                            match self.collect_inner(Bbid::fn_start(fn_def_id), arg) {
                                // guard will now be in function return local
                                GuardState::Returned => current_local = destination.local,
                                // guard dropped finish analysis
                                GuardState::Dropped => return guard_state.combine(GuardState::Dropped),
                                // function went into infinite loop, return
                                GuardState::Undetermined => return guard_state.combine(GuardState::Undetermined),
                            }
                        },
                        (None, Some(fn_def_id)) => {
                            // TODO
                        },
                        // we don't know what function was called, can't eximine if it locked anything
                        // FIXME: this might not be correct
                        (None, None) => (),
                    }

                    if let Some(target) = target {
                        basic_block = *target;
                    } else {
                        // function call diverges
                        return guard_state.combine(GuardState::Undetermined);
                    }
                },
                TerminatorKind::Assert { target, .. } => basic_block = *target,
                TerminatorKind::Yield { .. } => todo!(),
                // aparently this is like a return from generator?
                TerminatorKind::GeneratorDrop => todo!(),
                TerminatorKind::FalseEdge { real_target, .. } => basic_block = *real_target,
                TerminatorKind::FalseUnwind { real_target, .. } => basic_block = *real_target,
                // TODO: detect if inline asm operands is local we are using
                TerminatorKind::InlineAsm { destination, .. } => {
                    if let Some(dest) = destination {
                        basic_block = *dest;
                    } else {
                        // inline asm is diverging
                        return guard_state.combine(GuardState::Undetermined);
                    }
                }
            }
        }
    }
}

fn get_fn_def_id_from_terminator(terminator: &Terminator) -> Option<DefId> {
    let TerminatorKind::Call { func, .. } = &terminator.kind else {
        return None;
    };

    let Operand::Constant(c) = func else {
        return None;
    };

    let Const::Val(ConstValue::ZeroSized, fn_type) = c.const_ else {
        return None;
    };

    let TyKind::FnDef(def_id, _) = fn_type.kind() else {
        return None;
    };

    Some(*def_id)
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
                Rvalue::Aggregate(_, arguments) => {
                    for arg in arguments.iter() {
                        match arg {
                            // FIXME: I don't know if this is actually true, I think after drop elaboration
                            // the compiler may turn moves into copies
                            Operand::Copy(place) if place.local == current_local => panic!("lock guard cannot be copied"),
                            Operand::Move(place) if place.local == current_local => return assign_data.0.local,
                            _ => continue,
                        }
                    }

                    // none of the args to adt are the current local, so current local has not changed places
                    return current_local;
                },
                // the rest of rvalues for the most part won't be used on something like a lock guard
                _ => return current_local,
            };

            match from_operand {
                // FIXME: I don't know if this is actually true, I think after drop elaboration
                // the compiler may turn moves into copies
                Operand::Copy(place) if place.local == current_local => panic!("lock guard cannot be copied"),
                Operand::Move(place) if place.local == current_local => assign_data.0.local,
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