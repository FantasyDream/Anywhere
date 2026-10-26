//! Runtime-neutral state projection. A plugin sees one approved object, never the graph.
use super::{broker::error, projection, WebDocumentBinding, WebMessageSource};
use crate::application::Application;
use anywhere_protocol::{web_bridge::*, web_plugin::*};
use serde_json::{json, Value};

impl Application {
    pub fn web_snapshot(
        &self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
    ) -> WebResult<PluginSnapshot> {
        // Same lock order as activation, suspension and catalog publication.
        let session = self
            .session()
            .map_err(|_| error(WebErrorCode::HostFailure))?;
        let mut web = self
            .web_plugins
            .lock()
            .map_err(|_| error(WebErrorCode::HostFailure))?;
        let invocation = web
            .authorize(binding, source)
            .map_err(|_| error(WebErrorCode::Inactive))?;
        if session.state.session_id != invocation.session_id || session.suspended {
            return Err(error(WebErrorCode::Inactive));
        }
        let launch_revision = invocation.launch_revision;
        let mut context = invocation.input.input.clone();
        let plugin = invocation.command.plugin_id.clone();
        let permissions = &web.catalog.entries[&plugin].manifest.permissions;
        if session.state.revision > launch_revision {
            context = session
                .resolution
                .committed_contexts
                .iter()
                .find(|ctx| {
                    ctx.source == anywhere_protocol::ContextSource::UserInput
                        && permissions.iter().any(|permission| {
                            matches!(permission,
                        WebPermission::ContextRead { types } if types.contains(&ctx.type_id))
                        })
                })
                .map(|ctx| projection::build_with_resources(ctx, permissions).map(|v| v.0))
                .transpose()
                .map_err(|_| error(WebErrorCode::HostFailure))?
                .unwrap_or(WebInput::Empty {
                    reason: WebInputReason::NoObject,
                });
        }
        Ok(PluginSnapshot {
            version: (u64::from(session.state.revision) << 32)
                | u64::from(session.state.snapshot_seq),
            context,
            input: permissions
                .contains(&WebPermission::InputRead {})
                .then(|| session.state.input.clone()),
        })
    }

    /// A single latest-value wait per document. No periodic cross-layer state polling.
    /// Large snapshots use the existing bounded/chunked RPC path after a notification.
    pub async fn wait_web_state(
        &self,
        binding: &WebDocumentBinding,
        after: u64,
    ) -> WebResult<Option<String>> {
        let mut changes = self.resolver.latest_tx.subscribe();
        let source = || WebMessageSource {
            runtime_id: binding.runtime_id(),
            document_epoch: binding.document_epoch(),
            origin: binding.origin(),
            main_frame: true,
        };
        let mut closed = self
            .web_plugins
            .lock()
            .map_err(|_| error(WebErrorCode::HostFailure))?
            .authorize(binding, source())
            .map_err(|_| error(WebErrorCode::Inactive))?
            .closed
            .subscribe();
        loop {
            let state = self.web_snapshot(binding, source())?;
            if state.version > after {
                let mut event =
                    json!({"event":"state.changed", "version":state.version, "state":state});
                if serde_json::to_vec(&event)
                    .map_err(|_| error(WebErrorCode::HostFailure))?
                    .len()
                    > MAX_FRAME_BYTES
                {
                    event.as_object_mut().expect("event object").remove("state");
                }
                return Ok(Some(event.to_string()));
            }
            drop(state);
            let changed = tokio::select! {
                _ = closed.changed() => return Err(error(WebErrorCode::Inactive)),
                result = tokio::time::timeout(std::time::Duration::from_secs(15), changes.changed()) => result,
            };
            if changed.is_err() {
                self.validate_web_document(binding, source())
                    .map_err(|_| error(WebErrorCode::Inactive))?;
                return Ok(None);
            }
        }
    }

    pub(super) async fn set_web_input(
        &self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
        text: String,
        control: &std::sync::Arc<super::rpc_state::RequestControl>,
    ) -> WebResult<Value> {
        control.check()?;
        self.validate_web_document(binding, source)
            .map_err(|_| error(WebErrorCode::Inactive))?;
        let (id, revision, scope, mode) = {
            let session = self
                .session()
                .map_err(|_| error(WebErrorCode::HostFailure))?;
            if session.suspended {
                return Err(error(WebErrorCode::Inactive));
            }
            (
                session.state.session_id,
                session
                    .state
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| error(WebErrorCode::Expired))?,
                session.state.selected_scope.clone(),
                session.input_mode,
            )
        };
        let web = self.web_plugins.clone();
        let binding = binding.clone();
        let control = control.clone();
        let authorize = std::sync::Arc::new(move || {
            control.check().is_ok()
                && web.lock().is_ok_and(|mut service| {
                    service
                        .require(
                            &binding,
                            WebMessageSource {
                                runtime_id: binding.runtime_id(),
                                document_epoch: binding.document_epoch(),
                                origin: binding.origin(),
                                main_frame: true,
                            },
                            WebPermission::InputWrite {},
                        )
                        .is_ok()
                })
        });
        self.update_input_request(
            id,
            text,
            revision,
            crate::application::update_query::QueryOptions {
                scope,
                mode,
                filter: anywhere_protocol::FileSearchFilter::All,
                authorize: Some(authorize),
            },
        )
        .await
        .map_err(|_| error(WebErrorCode::HostFailure))?
        .ok_or_else(|| error(WebErrorCode::Expired))?;
        Ok(Value::Null)
    }
}
