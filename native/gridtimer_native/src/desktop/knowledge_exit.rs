// v1.1.0.8 Windows - Await the exact note receipt without blocking editor frames.
struct DesktopDocumentExit {
    workspace: String,
    note_id: String,
    editor_epoch: u64,
    revision: u64,
    started: Instant,
    property_inputs: Vec<(String, String, String)>,
}

impl TimerWindowsClient {
    fn document_exit_property_inputs(&self) -> Vec<(String, String, String)> {
        let mut inputs = self
            .desktop_ui
            .knowledge
            .cell_inputs
            .iter()
            .filter(|(key, input)| {
                (key.starts_with("page:") || key.starts_with("table:"))
                    && input.text != input.source
            })
            .map(|(key, input)| (key.clone(), input.source.clone(), input.text.clone()))
            .collect::<Vec<_>>();
        inputs.sort();
        inputs
    }

    fn finish_document_exit(&mut self) {
        self.desktop_ui.navigation.pending_exit = None;
        self.desktop_ui.navigation.editor_visible = false;
        self.desktop_ui.note_editor_focus = false;
        self.desktop_ui.navigation.forget_document_text_edits();
    }

    fn poll_document_exit(&mut self) {
        let Some(exit) = self.desktop_ui.navigation.pending_exit.as_ref() else {
            return;
        };
        let current = exit.workspace == self.background_job_workspace_fingerprint()
            && exit.note_id == self.selected_note_id
            && exit.editor_epoch == self.desktop_ui.navigation.editor_epoch
            && exit.revision == self.persistence.revisions.note
            && exit.property_inputs == self.document_exit_property_inputs()
            && matches!(self.tab, AppTab::Knowledge | AppTab::Notes)
            && matches!(self.shutdown_state, ClientShutdownState::Running)
            && self.workspace_persistence_ready
            && !self.workspace_edit_locked();
        if !current || self.persistence.failed() || self.selected_note_is_locked() {
            self.desktop_ui.navigation.pending_exit = None;
            return;
        }
        if self.note_dirty {
            if let Some(ctx) = self.sync_apply.frame_context.clone() {
                self.submit_draft_snapshot(&ctx, true);
                ctx.request_repaint_after(Duration::from_millis(50));
            }
            return;
        }
        // Database inputs have their own validation. An invalid or failed
        // property commit retains the editor and the unsaved property text.
        match self.flush_knowledge_property_inputs() {
            Ok(()) => self.finish_document_exit(),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => self.desktop_ui.navigation.pending_exit = None,
        }
    }

    fn ui_document_exit_wait(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(exit) = self.desktop_ui.navigation.pending_exit.as_ref() else {
            return false;
        };
        let slow = exit.started.elapsed() >= Duration::from_secs(8);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(if slow {
                "本地仍在写入，完成后返回"
            } else {
                "保存后返回"
            });
            if ui.button("留在此页").clicked() {
                // Cancel navigation only. A submitted durable write is allowed
                // to finish and its normal revision guard still applies.
                self.desktop_ui.navigation.pending_exit = None;
            }
        });
        ui.ctx().request_repaint_after(Duration::from_millis(50));
        true
    }
}
