//! Logical command plus the Host catalog qualification captured at registration.
//! This is not serializable and is never constructed from a Web message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebCommandRef {
    pub(crate) plugin_id: String,
    pub(crate) command_id: String,
    pub(crate) registration: String,
    pub(crate) input_matchers: std::sync::Arc<[anywhere_protocol::web_plugin::WebInputMatcher]>,
}

impl WebCommandRef {
    pub fn action_id(&self) -> String {
        format!("web:{}:{}", self.plugin_id, self.command_id)
    }

    pub fn matches_input(
        &self,
        matches: &mut crate::core::context::input_patterns::InputPatternMatches<'_>,
    ) -> bool {
        matches.any(&self.input_matchers)
    }
}

impl super::descriptor::ActionRegistry {
    /// Only the published, enabled command catalog can opt a captured text into
    /// content mode. Recognition performs no WebView startup or permission reads.
    pub(crate) fn recognizes_web_input(&self, text: &str) -> bool {
        let mut matches = crate::core::context::input_patterns::InputPatternMatches::new(text);
        self.commands().any(|action| match &action.executor {
            super::descriptor::ActionExecutor::WebCommand(command) => {
                command.matches_input(&mut matches)
            }
            _ => false,
        })
    }
}
