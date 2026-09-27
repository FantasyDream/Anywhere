//! JSON document actions.
use crate::core::action::availability::AvailabilityRule;
use crate::core::action::bundle::{ActionBundle, ActionSpec};
use crate::core::action::group::groups;
use crate::core::action::handler::{ActionHandler, ExecutionOutput};
use crate::core::context::{ContextValue, ResolvedContext};
use anyhow::{bail, Result};
use anywhere_platform::Platform;
use anywhere_protocol::*;

#[derive(Clone, Copy)]
enum JsonMode {
    Pretty,
    Minify,
    CopyPretty,
}

#[derive(Clone, Copy)]
struct Json {
    mode: JsonMode,
}

impl ActionHandler for Json {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        let ContextValue::Json { parsed, .. } = &context.value else {
            bail!("JSON context required");
        };
        let rendered = if matches!(self.mode, JsonMode::Minify) {
            serde_json::to_string(parsed.as_ref())?
        } else {
            serde_json::to_string_pretty(parsed.as_ref())?
        };
        if matches!(self.mode, JsonMode::CopyPretty) {
            platform.copy_text(&rendered)?;
            return Ok(ExecutionOutput::dismiss());
        }
        Ok(ExecutionOutput::preview(rendered))
    }
}

const FORMAT: Json = Json {
    mode: JsonMode::Pretty,
};
const MINIFY: Json = Json {
    mode: JsonMode::Minify,
};
const COPY: Json = Json {
    mode: JsonMode::CopyPretty,
};

pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: groups::json(),
        actions: vec![
            ActionSpec {
                id: "core.json.format",
                title: "Format JSON",
                accepts: JSON_LEGACY,
                handler: &FORMAT,
                availability: AvailabilityRule::Enabled,
                base_priority: 30,
                icon: "code",
                aliases: &["pretty", "beautify"],
                keywords: &["document"],
            },
            ActionSpec {
                id: "core.json.minify",
                title: "Minify JSON",
                accepts: JSON_LEGACY,
                handler: &MINIFY,
                availability: AvailabilityRule::Enabled,
                base_priority: 20,
                icon: "code",
                aliases: &["compact"],
                keywords: &["document"],
            },
            ActionSpec {
                id: "core.json.copy",
                title: "Copy Formatted JSON",
                accepts: JSON_LEGACY,
                handler: &COPY,
                availability: AvailabilityRule::Enabled,
                base_priority: 10,
                icon: "copy",
                aliases: &["copy", "clipboard"],
                keywords: &["document"],
            },
        ],
    }
}
