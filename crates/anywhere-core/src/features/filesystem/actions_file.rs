//! File actions.
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
        group: groups::file(),
        actions: vec![
            ActionSpec {
                id: "core.file.open",
                title: "Open File",
                accepts: FILE,
                executor: BuiltinExecutor::OpenFile,
                base_priority: 30,
                icon: "file",
                aliases: &["open", "edit"],
                keywords: &["document"],
            },
            ActionSpec {
                id: "core.file.reveal",
                title: "Reveal in File Manager",
                accepts: FILE,
                executor: BuiltinExecutor::RevealFile,
                base_priority: 20,
                icon: "folder",
                aliases: &["reveal", "finder", "show"],
                keywords: &["document"],
            },
        ],
    }
}
