use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use rustc_session::Session;
use rustc_middle::ty::Ty;
use rustc_span::Span;
use rustc_error_messages::MultiSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorStatus {
    Ok,
    DeadlockDetected,
}

impl ErrorStatus {
    pub fn error_emitted(&self) -> bool {
        matches!(self, ErrorStatus::DeadlockDetected)
    }
}

pub struct Errors<'tcx> {
    session: Rc<Session>,
    // this ensures errors are emitted in order
    errors: RefCell<BTreeSet<DeadlockError<'tcx>>>,
}

impl<'tcx> Errors<'tcx> {
    pub fn new(session: Rc<Session>) -> Self {
        Errors {
            session,
            errors: RefCell::default(),
        }
    }

    pub fn emit_deadlock_error(&self, parent_invocation: InvocationErrorInfo<'tcx>, child_invocation: InvocationErrorInfo<'tcx>) {
        let error = DeadlockError {
            parent_invocation,
            child_invocation,
        };

        self.errors.borrow_mut().insert(error);
    }

    pub fn emit_all_errors(&self) -> ErrorStatus {
        for error in self.errors.borrow().iter() {
            let mut multi_span = MultiSpan::from_span(error.child_invocation.span);
            multi_span.push_span_label(error.parent_invocation.span, format!("lock class `{}` first locked here", error.parent_invocation.ty));
            multi_span.push_span_label(error.child_invocation.span, format!("deadlock occurs when lock class `{}` locked here", error.child_invocation.ty));
        
            self.session.struct_span_err(multi_span, "potential deadlock detected").emit();
        }

        if self.errors.borrow().len() > 0 {
            ErrorStatus::DeadlockDetected
        } else {
            ErrorStatus::Ok
        }
    }
}

pub struct InvocationErrorInfo<'tcx> {
    pub span: Span,
    pub ty: Ty<'tcx>
}

struct DeadlockError<'tcx> {
    parent_invocation: InvocationErrorInfo<'tcx>,
    child_invocation: InvocationErrorInfo<'tcx>,
}

impl PartialEq for DeadlockError<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.child_invocation.span == other.child_invocation.span
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
        self.child_invocation.span.cmp(&other.child_invocation.span)
    }
}