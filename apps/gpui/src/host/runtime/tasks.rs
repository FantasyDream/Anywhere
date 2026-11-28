use super::*;

impl LiveLauncher {
    pub fn follow_executions(&self) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let (sender, receiver) = channel(1);
        self.runtime().spawn(async move {
            let mut updates = application.execution_updates();
            loop {
                let rows = match application.execution_sessions() {
                    Ok(rows) => rows.into_iter().map(execution_row).collect(),
                    Err(error) => {
                        let _ = sender.send(LiveEvent::Error(format!("{error:#}"))).await;
                        break;
                    }
                };
                if sender.send(LiveEvent::Executions(rows)).await.is_err() {
                    break;
                }
                tokio::select! {
                    _closed = sender.closed() => break,
                    changed = updates.changed() => {
                        if changed.is_err() {
                            break;
                        }
                    }
                }
                tokio::select! {
                    _closed = sender.closed() => break,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(40)) => {}
                }
                let _ = updates.borrow_and_update();
            }
        });
        receiver
    }

    pub fn execution_detail(&self, id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            application
                .execution_session(&id)?
                .map(|detail| LiveEvent::ExecutionDetail(execution_detail(detail)))
                .ok_or_else(|| anyhow::anyhow!("execution session not found"))
        })
    }

    pub fn open_execution_directory(&self, id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            let detail = application
                .execution_session(&id)?
                .map(execution_detail)
                .ok_or_else(|| anyhow::anyhow!("execution session not found"))?;
            let directory = std::path::Path::new(&detail.row.directory);
            anyhow::ensure!(
                directory.is_absolute() && directory.is_dir(),
                "working directory unavailable"
            );
            application.platform().open_directory(directory)?;
            Ok(LiveEvent::Ignored)
        })
    }

    pub fn stop_execution(&self, id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            application.stop_execution_session(&id)?;
            Ok(LiveEvent::Ignored)
        })
    }

    pub fn remove_execution(&self, id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            application.remove_execution_session(&id)?;
            Ok(LiveEvent::Ignored)
        })
    }
}
