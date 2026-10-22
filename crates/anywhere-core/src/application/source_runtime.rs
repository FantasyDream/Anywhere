//! Generic bounded execution for streaming sources; no file/workspace protocol.
use crate::core::{
    context::ExecutionContext,
    search::candidate_source::{
        AsyncCandidateBatch, AsyncCandidateSource, AsyncSearchRequest, BatchUpdate,
    },
};
use anywhere_protocol::SearchSourceState;
use std::sync::Arc;
type Publish = Arc<dyn Fn(AsyncCandidateBatch) + Send + Sync>;
fn unavailable() -> AsyncCandidateBatch {
    AsyncCandidateBatch {
        update: BatchUpdate::Replace,
        candidates: Vec::new(),
        state: SearchSourceState::Unavailable,
        metadata: None,
    }
}
pub(super) fn spawn_source(
    source: Arc<dyn AsyncCandidateSource>,
    request: AsyncSearchRequest,
    ticket: ExecutionContext,
    workers: Arc<tokio::sync::Semaphore>,
    valid: Arc<dyn Fn() -> bool + Send + Sync>,
    publish: Publish,
) {
    tokio::spawn(async move {
        tokio::select! { _=tokio::time::sleep(source.debounce())=>{}, _=ticket.superseded()=>return }
        let _permit = tokio::select! { p=workers.acquire_owned()=>match p {Ok(p)=>p,Err(_)=>return}, _=ticket.superseded()=>return };
        if !ticket.is_current() || !valid() {
            return;
        }
        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        let stream = source.stream(request, ticket.clone(), tx);
        let work = async {
            if let Some(timeout) = source.timeout() {
                tokio::time::timeout(timeout, stream)
                    .await
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("search source timed out")))
            } else {
                stream.await
            }
        };
        tokio::pin!(work);
        let mut closed = false;
        loop {
            tokio::select! {
                batch=rx.recv(), if !closed => match batch {
                    Some(batch)=>{ if ticket.is_current() && valid() {publish(batch);} },
                    None=> {closed = true;}
                },
                result=&mut work=> {
                    while let Ok(batch)=rx.try_recv(){if ticket.is_current()&&valid(){publish(batch);}}
                    if let Err(error)=result{tracing::warn!(source=source.id(),%error,"search_source_unavailable");if ticket.is_current()&&valid(){publish(unavailable());}}
                    break;
                },
                _=ticket.superseded()=>return,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Closing {
        entered: tokio::sync::Notify,
    }
    #[async_trait::async_trait]
    impl AsyncCandidateSource for Closing {
        fn id(&self) -> &'static str {
            "closing"
        }
        fn debounce(&self) -> std::time::Duration {
            std::time::Duration::ZERO
        }
        fn timeout(&self) -> Option<std::time::Duration> {
            None
        }
        async fn candidates(
            &self,
            _: &str,
            _: ExecutionContext,
        ) -> anyhow::Result<Vec<crate::core::search::candidate::Candidate>> {
            unreachable!()
        }
        async fn stream(
            &self,
            _: AsyncSearchRequest,
            _: ExecutionContext,
            tx: tokio::sync::mpsc::Sender<AsyncCandidateBatch>,
        ) -> anyhow::Result<()> {
            drop(tx);
            self.entered.notify_one();
            std::future::pending().await
        }
    }
    #[tokio::test]
    async fn closed_channel_cleanup_does_not_bypass_cancellation_or_leak_admission() {
        let source = Arc::new(Closing {
            entered: tokio::sync::Notify::new(),
        });
        let ticket = ExecutionContext::detached(1, 0);
        let stamp = ticket.current.clone();
        let workers = Arc::new(tokio::sync::Semaphore::new(1));
        spawn_source(
            source.clone(),
            AsyncSearchRequest {
                text: "q".into(),
                options: None,
            },
            ticket,
            workers.clone(),
            Arc::new(|| true),
            Arc::new(|_| panic!("cancelled producer must not publish")),
        );
        source.entered.notified().await;
        stamp.store(0, std::sync::atomic::Ordering::Release);
        let permit = tokio::time::timeout(std::time::Duration::from_secs(1), workers.acquire())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        assert_eq!(workers.available_permits(), 1);
    }
}
