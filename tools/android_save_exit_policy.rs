//! State policy for the two-stage Android editor exit: dismiss IME, then leave editor.
pub fn should_send_editor_exit_back(is_parent: bool, editor_visible: bool, back_sent: bool) -> bool {
    !is_parent && editor_visible && !back_sent
}
