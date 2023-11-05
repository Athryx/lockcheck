use std::panic::{catch_unwind, AssertUnwindSafe, take_hook, set_hook};

use rustc_span::def_id::DefId;
use rustc_middle::mir::Body;
use rustc_middle::ty::TyCtxt;

pub trait TyCtxtExt<'tcx> {
    fn try_optimized_mir(self, def_id: DefId) -> Option<&'tcx Body<'tcx>>;
}

impl<'tcx> TyCtxtExt<'tcx> for TyCtxt<'tcx> {
    // FIXME: this is an ugly hack to get optimized_mir without panicing if it doesn't exist
    // as far as I can tell, tctxt does not give us a version of optimized_mir that returns option instead of panicing
    fn try_optimized_mir(self, def_id: DefId) -> Option<&'tcx Body<'tcx>> {
        let hir = self.hir();

        if let Some(local_def_id) = def_id.as_local() {
            // this will cause internal compiler error to print if we try to get mir
            // of local definition wich has no body, so check here first
            if hir.maybe_body_owned_by(local_def_id) == None {
                return None;
            }
        }

        let tcx = AssertUnwindSafe(self);

        let prev_hook = take_hook();
        set_hook(Box::new(|_| {}));

        let body = catch_unwind(|| tcx.optimized_mir(def_id)).ok();

        set_hook(prev_hook);

        body
    }
}