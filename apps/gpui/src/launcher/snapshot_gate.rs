use anywhere_protocol::{LauncherState, Revision, SessionId};

/// Orders snapshots for one active launcher session.
///
/// Core may publish several snapshots for the same revision as background
/// context arrives. A request result is accepted once its requested revision is
/// visible, while later snapshots are accepted only when their monotonic cursor
/// advances.
#[derive(Clone, Debug)]
pub struct SnapshotGate {
    session_id: SessionId,
    minimum_revision: Revision,
    cursor: Option<u64>,
}

impl SnapshotGate {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            minimum_revision: 0,
            cursor: None,
        }
    }

    pub fn begin_request(&mut self, revision: Revision) {
        self.minimum_revision = self.minimum_revision.max(revision);
    }

    pub fn accept_request(&mut self, state: &LauncherState) -> bool {
        self.accept(state, self.minimum_revision)
    }

    pub fn accept_update(&mut self, state: &LauncherState) -> bool {
        self.accept(state, self.minimum_revision)
    }

    pub fn cursor(&self) -> u64 {
        self.cursor.unwrap_or(0)
    }

    pub fn cursor_for(state: &LauncherState) -> u64 {
        (u64::from(state.revision) << 32) | u64::from(state.snapshot_seq)
    }

    fn accept(&mut self, state: &LauncherState, minimum_revision: Revision) -> bool {
        let cursor = Self::cursor_for(state);
        if state.session_id != self.session_id
            || state.revision < minimum_revision
            || self.cursor.is_some_and(|current| cursor <= current)
        {
            return false;
        }
        self.cursor = Some(cursor);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(session: SessionId, revision: Revision, snapshot_seq: u32) -> LauncherState {
        LauncherState {
            session_id: session,
            revision,
            snapshot_seq,
            ..Default::default()
        }
    }

    #[test]
    fn accepts_only_monotonic_snapshots_for_the_reserved_session() {
        let mut gate = SnapshotGate::new(7);
        assert!(!gate.accept_update(&state(8, 1, 2)));
        assert!(gate.accept_update(&state(7, 1, 2)));
        assert!(!gate.accept_update(&state(7, 1, 1)));
        assert!(gate.accept_update(&state(7, 1, 3)));
        assert!(gate.accept_update(&state(7, 2, 0)));
        assert!(!gate.accept_update(&state(7, 1, 9)));
    }

    #[test]
    fn accepts_the_initial_zero_cursor_once() {
        let mut gate = SnapshotGate::new(7);
        assert_eq!(gate.cursor(), 0);
        assert!(gate.accept_update(&state(7, 0, 0)));
        assert_eq!(gate.cursor(), 0);
        assert!(!gate.accept_update(&state(7, 0, 0)));
    }

    #[test]
    fn a_new_request_rejects_snapshots_before_its_revision() {
        let mut gate = SnapshotGate::new(3);
        assert!(gate.accept_update(&state(3, 2, 0)));
        gate.begin_request(4);
        assert!(!gate.accept_request(&state(3, 3, 20)));
        assert!(gate.accept_request(&state(3, 4, 0)));
        assert!(gate.accept_update(&state(3, 4, 1)));
    }
}
