//! Per-document bounded bookkeeping. Dropping/rebinding cancels all unfinished requests.
use anywhere_protocol::web_bridge::*;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::time::{Duration, Instant};

pub(super) struct RequestControl {
    cancelled: AtomicBool,
    cancellation: tokio::sync::Notify,
    pub running: AtomicBool,
    pub deadline: Instant,
}
impl RequestControl {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.cancellation.notify_waiters();
    }
    pub async fn cancelled(&self) {
        let notified = self.cancellation.notified();
        tokio::pin!(notified);
        // Register before checking the flag so cancellation between the check
        // and the await cannot be lost, including on a different runtime thread.
        notified.as_mut().enable();
        if !self.cancelled.load(Ordering::Acquire) {
            notified.await;
        }
    }
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
    seen_order: VecDeque<String>,
    business_requests: usize,
    pub pending: BTreeMap<String, Arc<RequestControl>>,
    pub transfer: Option<Transfer>,
}
impl Drop for RpcState {
    fn drop(&mut self) {
        for request in self.pending.values() {
            request.cancel();
        }
    }
}
impl RpcState {
    #[cfg(test)]
    pub fn admit(&mut self, id: &str, queued: bool) -> WebResult<RequestTicket> {
        self.admit_with_quota(id, queued, true)
    }

    pub fn admit_call(&mut self, id: &str, call: &WebCall) -> WebResult<RequestTicket> {
        let queued = !matches!(call, WebCall::Cancel { .. });
        let counts_quota = !matches!(
            call,
            WebCall::TextBegin { .. }
                | WebCall::TextChunk { .. }
                | WebCall::TextCommit { .. }
                | WebCall::TextRead { .. }
                | WebCall::TextCancel { .. }
                | WebCall::Cancel { .. }
        );
        self.admit_with_quota(id, queued, counts_quota)
    }

    fn admit_with_quota(
        &mut self,
        id: &str,
        queued: bool,
        counts_quota: bool,
    ) -> WebResult<RequestTicket> {
        self.prune();
        if self.seen.contains(id) {
            return Err(WebError::new(WebErrorCode::DuplicateRequest));
        }
        if counts_quota && self.business_requests >= MAX_REQUESTS {
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
        if self.seen_order.len() >= MAX_REQUESTS * 2 {
            if let Some(evicted) = self.seen_order.pop_front() {
                if !self.pending.contains_key(&evicted) {
                    self.seen.remove(&evicted);
                } else {
                    self.seen_order.push_back(evicted);
                }
            }
        }
        self.seen.insert(id.into());
        self.seen_order.push_back(id.into());
        if counts_quota {
            self.business_requests += 1;
        }
        let control = Arc::new(RequestControl {
            cancelled: AtomicBool::new(false),
            cancellation: tokio::sync::Notify::new(),
            running: AtomicBool::new(false),
            deadline: Instant::now() + Duration::from_millis(REQUEST_TIMEOUT_MS),
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
            request.cancel();
            return true;
        }
        false
    }
    pub fn prune(&mut self) {
        if self.transfer.as_ref().is_some_and(Transfer::expired) {
            self.transfer = None;
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
