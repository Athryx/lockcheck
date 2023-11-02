use rustc_span::Span;
use rustc_session::Session;
use rustc_error_messages::MultiSpan;
use rustc_middle::ty::Ty;

pub fn deadlock_error(session: &Session, parent_ty: Ty, parent_lock: Span, child_ty: Ty, child_lock: Span) {
    let mut multi_span = MultiSpan::from_span(child_lock);
    //multi_span.push_span_label(parent_lock, "first lock locked here");
    multi_span.push_span_label(parent_lock, format!("lock class `{parent_ty}` first locked here"));
    //multi_span.push_span_label(child_lock, "deadlock potentially occurs here");
    multi_span.push_span_label(child_lock, format!("deadlock occurs when lock class `{child_ty}` locked here"));

    session.struct_span_err(multi_span, "potential deadlock detected").emit();
}