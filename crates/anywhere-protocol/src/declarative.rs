use super::{
    extension,
    identifiers::{ActionId, ContextTypeId, GroupId},
    ui,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub protocol_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<extension::ExtensionCapability>,
    pub commands: Vec<PluginCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCommand {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub accepts: Vec<ContextTypeId>,
    #[serde(default)]
    pub base_priority: i32,
    #[serde(default)]
    pub group_id: Option<GroupId>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub action: DeclarativeAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeclarativeAction {
    OpenUrl { url: String },
    OpenFile { path: String },
    Copy { text: String },
    ExecuteBuiltin { command_id: ActionId },
    RenderDetail { view: ui::DetailView },
}
