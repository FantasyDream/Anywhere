//! Product group presentation belongs to first-party contributions.
pub use crate::core::action::group::groups::*;
use crate::core::action::group::ActionGroup;
pub fn git() -> ActionGroup {
    ActionGroup {
        id: GIT.into(),
        title: "Git".into(),
        icon: Some("git".into()),
    }
}

pub fn folder() -> ActionGroup {
    ActionGroup {
        id: FOLDER.into(),
        title: "Folder".into(),
        icon: Some("folder".into()),
    }
}

pub fn file() -> ActionGroup {
    ActionGroup {
        id: FILE.into(),
        title: "File".into(),
        icon: Some("file".into()),
    }
}

pub fn web() -> ActionGroup {
    ActionGroup {
        id: WEB.into(),
        title: "Web".into(),
        icon: Some("globe".into()),
    }
}

pub fn json() -> ActionGroup {
    ActionGroup {
        id: JSON.into(),
        title: "JSON".into(),
        icon: Some("code".into()),
    }
}

pub fn app() -> ActionGroup {
    ActionGroup {
        id: APP.into(),
        title: "Current App".into(),
        icon: None,
    }
}

/// The built-in catalogue, in the order the scope bar falls back to.
pub fn rust() -> ActionGroup {
    ActionGroup {
        id: RUST.into(),
        title: "Rust".into(),
        icon: Some("code".into()),
    }
}

pub fn builtin() -> Vec<ActionGroup> {
    vec![
        git(),
        folder(),
        file(),
        web(),
        json(),
        rust(),
        maven(),
        gradle(),
        node(),
        spring_boot(),
        app(),
    ]
}

pub fn maven() -> ActionGroup {
    project_group(MAVEN, "Maven")
}
pub fn gradle() -> ActionGroup {
    project_group(GRADLE, "Gradle")
}
pub fn spring_boot() -> ActionGroup {
    project_group(SPRING_BOOT, "Spring Boot")
}
pub fn node() -> ActionGroup {
    project_group(NODE, "Node")
}
fn project_group(id: &str, title: &str) -> ActionGroup {
    ActionGroup {
        id: id.into(),
        title: title.into(),
        icon: Some("code".into()),
    }
}
