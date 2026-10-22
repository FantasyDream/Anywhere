//! Per-document bounded bookkeeping. Dropping/rebinding cancels all unfinished requests.
use anywhere_protocol::web_bridge::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::time::{Duration, Instant};

pub(super) struct RequestControl {
    pub cancelled: AtomicBool,
    pub running: AtomicBool,
    pub deadline: Instant,
}
impl RequestControl {
    pub fn check(&self) -> WebResult<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(WebError::new(WebErrorCode::Cancelled));
        }
        if Instant::now() >= self.deadline {
            return Err(WebError::new(WebErrorCode::Timeout));
        }
        Ok(())
    }
}
#[derive(Clone)]
pub(super) struct RequestTicket {
    pub id: String,
    pub control: Arc<RequestControl>,
}
pub(super) enum ClipboardOperation {
    Read,
    Write(String),
}
pub(super) struct Confirmation {
    pub id: String,
    pub request: RequestTicket,
    pub operation: ClipboardOperation,
}

pub(super) enum Transfer {
    Upload {
        id: String,
        bytes: usize,
        sequence: u32,
        text: String,
        expires: Instant,
    },
    Download {
        id: String,
        sequence: u32,
        offset: usize,
        text: String,
        expires: Instant,
    },
}
impl Transfer {
    pub fn id(&self) -> &str {
        match self {
            Self::Upload { id, .. } | Self::Download { id, .. } => id,
        }
    }
    pub fn expired(&self) -> bool {
        Instant::now()
            >= match self {
                Self::Upload { expires, .. } | Self::Download { expires, .. } => *expires,
            }
    }
}

#[derive(Default)]
pub(super) struct RpcState {
    seen: BTreeSet<String>,
    pub pending: BTreeMap<String, Arc<RequestControl>>,
    pub transfer: Option<Transfer>,
    pub confirmation: Option<Confirmation>,
}
impl Drop for RpcState {
    fn drop(&mut self) {
        for request in self.pending.values() {
            request.cancelled.store(true, Ordering::Release);
        }
    }
}
impl RpcState {
    pub fn admit(
        &mut self,
        id: &str,
        confirmation: bool,
        queued: bool,
    ) -> WebResult<RequestTicket> {
        self.prune();
        if self.seen.contains(id) {
            return Err(WebError::new(WebErrorCode::DuplicateRequest));
        }
        if self.seen.len() >= MAX_REQUESTS {
            return Err(WebError::new(WebErrorCode::QuotaExceeded));
        }
        if queued
            && (self.pending.len() >= MAX_PENDING
                || self
                    .pending
                    .values()
                    .filter(|c| !c.running.load(Ordering::Acquire))
                    .count()
                    >= MAX_PENDING - MAX_RUNNING)
        {
            return Err(WebError::new(WebErrorCode::Busy));
        }
        self.seen.insert(id.into());
        let duration = if confirmation {
            CONFIRM_TIMEOUT_MS
        } else {
            REQUEST_TIMEOUT_MS
        };
        let control = Arc::new(RequestControl {
            cancelled: AtomicBool::new(false),
            running: AtomicBool::new(false),
            deadline: Instant::now() + Duration::from_millis(duration),
        });
        if queued {
            self.pending.insert(id.into(), control.clone());
        }
        Ok(RequestTicket {
            id: id.into(),
            control,
        })
    }
    pub fn cancel(&mut self, id: &str) -> bool {
        if let Some(request) = self.pending.get(id) {
            request.cancelled.store(true, Ordering::Release);
            if self
                .confirmation
                .as_ref()
                .is_some_and(|c| c.request.id == id)
            {
                self.confirmation = None;
                self.pending.remove(id);
            }
            return true;
        }
        false
    }
    pub fn prune(&mut self) {
        if self.transfer.as_ref().is_some_and(Transfer::expired) {
            self.transfer = None;
        }
        if self
            .confirmation
            .as_ref()
            .is_some_and(|c| c.request.control.check().is_err())
        {
            if let Some(confirmation) = self.confirmation.take() {
                self.pending.remove(&confirmation.request.id);
            }
        }
    }
    pub fn finish(&mut self, ticket: &RequestTicket) {
        if self
            .pending
            .get(&ticket.id)
            .is_some_and(|c| Arc::ptr_eq(c, &ticket.control))
        {
            self.pending.remove(&ticket.id);
        }
    }
}
