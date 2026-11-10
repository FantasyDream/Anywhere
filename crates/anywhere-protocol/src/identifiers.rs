pub type SessionId = u32;
pub type Revision = u32;
pub type ContextId = String;
pub type ContextTypeId = String;
pub type ProviderId = String;
/// Identity of one action, as the plugin manifest and the DTOs spell it.
pub type ActionId = String;
/// Identity of one row in the list: an action *or* an application, with its object.
pub type CandidateId = String;
pub type GroupId = String;
/// Stable semantic identity of a context, independent of snapshot row numbering.
pub type ContextKey = String;
/// A process lifetime, independent of Launcher SessionId and query Revision.
pub type ExecutionSessionId = String;
pub const PROJECT_RUST: &str = "project.rust";
pub const PROJECT_MAVEN: &str = "project.maven";
pub const PROJECT_GRADLE: &str = "project.gradle";
pub const JAVA_SPRING_BOOT: &str = "java.spring_boot";
pub const PROJECT_NODE: &str = "project.node";
