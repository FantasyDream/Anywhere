//! JSON document actions.
//!
//! Actions are declared as a bundle: the group they appear under plus the actions
//! themselves. Registering the bundle is all it takes for the feature to show up in search
//! and in the scope bar; nothing in the engine, the search pipeline or the launcher service
//! knows this feature exists.
use crate::core::action::bundle::{ActionBundle, ActionSpec};
use crate::core::action::descriptor::BuiltinExecutor;
use crate::core::action::group::groups;
use anywhere_protocol::*;

pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: groups::json(),
        actions: vec![
            ActionSpec {
                id: "core.json.format",
                title: "Format JSON",
                accepts: JSON_LEGACY,
                executor: BuiltinExecutor::FormatJson,
                base_priority: 30,
                icon: "code",
                aliases: &["pretty", "beautify"],
                keywords: &["document"],
            },
            ActionSpec {
                id: "core.json.minify",
                title: "Minify JSON",
                accepts: JSON_LEGACY,
                executor: BuiltinExecutor::MinifyJson,
                base_priority: 20,
                icon: "code",
                aliases: &["compact"],
                keywords: &["document"],
            },
            ActionSpec {
                id: "core.json.copy",
                title: "Copy Formatted JSON",
                accepts: JSON_LEGACY,
                executor: BuiltinExecutor::CopyFormattedJson,
                base_priority: 10,
                icon: "copy",
                aliases: &["copy", "clipboard"],
                keywords: &["document"],
            },
        ],
    }
}
