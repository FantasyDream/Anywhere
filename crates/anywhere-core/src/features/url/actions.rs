//! URL actions.
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
        group: groups::web(),
        actions: vec![
            ActionSpec {
                id: "core.url.open",
                title: "Open in Default Browser",
                accepts: URL,
                executor: BuiltinExecutor::OpenBrowser,
                base_priority: 30,
                icon: "globe",
                aliases: &["browser", "open"],
                keywords: &["link"],
            },
            ActionSpec {
                id: "core.url.copy",
                title: "Copy URL",
                accepts: URL,
                executor: BuiltinExecutor::CopyUrl,
                base_priority: 20,
                icon: "copy",
                aliases: &["copy", "link", "clipboard"],
                keywords: &["url"],
            },
        ],
    }
}
