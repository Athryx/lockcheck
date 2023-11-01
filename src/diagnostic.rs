use rustc_session::Session;

pub fn deadlock_error(session: &Session) {
    session.struct_err("deadlock detected").emit();
}