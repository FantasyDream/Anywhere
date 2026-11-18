use crate::launcher::SnapshotGate;
use anywhere_protocol::SessionId;
use tokio::sync::oneshot;

#[derive(Default)]
pub(super) struct LiveState {
    pub session_id: Option<SessionId>,
    pub request_revision: u32,
    pub generation: u64,
    pub cancel: Option<oneshot::Sender<()>>,
    pub snapshot_gate: Option<SnapshotGate>,
    pub awaiting_query_layout: bool,
}

impl LiveState {
    pub(super) fn begin_stream(&mut self) -> (u64, oneshot::Receiver<()>) {
        self.cancel_stream();
        self.generation = self.generation.wrapping_add(1);
        let (cancel, cancelled) = oneshot::channel();
        self.cancel = Some(cancel);
        (self.generation, cancelled)
    }

    pub(super) fn cancel_stream(&mut self) {
        self.awaiting_query_layout = false;
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}
