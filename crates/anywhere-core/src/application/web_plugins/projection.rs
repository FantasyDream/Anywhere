//! Project one selected object. Never export a graph, provider properties or raw paths.
use crate::core::context::{ContextValue, ResolvedContext};
use crate::features::tools::ENTRY_CONTEXT;
use anyhow::Result;
use anywhere_protocol::{
    web_plugin::{WebInput, WebInputReason, WebPermission, WebResource, WebResourceKind},
    *,
};
use std::{collections::BTreeMap, path::PathBuf};

use anywhere_protocol::web_bridge::MAX_TEXT_BYTES;
const MAX_RESOURCES: usize = 128;
pub(super) type Resources = BTreeMap<String, PathBuf>;

pub(super) fn build_with_resources(
    context: &ResolvedContext,
    permissions: &[WebPermission],
) -> Result<(WebInput, Resources)> {
    let empty = |reason| (WebInput::Empty { reason }, BTreeMap::new());
    if context.type_id == ENTRY_CONTEXT {
        return Ok(empty(WebInputReason::NoObject));
    }
    let granted = permissions.iter().any(|permission| matches!(permission, WebPermission::ContextRead { types } if types.contains(&context.type_id)));
    if !granted {
        return Ok(empty(WebInputReason::PermissionDenied));
    }
    let source = context.source;
    let input = match (&*context.type_id, &context.value) {
        (TEXT, ContextValue::Text(text)) if text.len() <= MAX_TEXT_BYTES => WebInput::Text {
            source,
            text: text.to_string(),
        },
        (URL, ContextValue::Url(url)) if url.as_str().len() <= MAX_TEXT_BYTES => WebInput::Url {
            source,
            url: url.to_string(),
        },
        (TEXT, ContextValue::Text(_)) | (URL, ContextValue::Url(_)) => {
            return Ok(empty(WebInputReason::TooLarge))
        }
        (FILE, ContextValue::File(path)) | (DIRECTORY, ContextValue::Directory(path)) => {
            let kind = if context.type_id == FILE {
                WebResourceKind::File
            } else {
                WebResourceKind::Directory
            };
            return resources(source, [(path, kind)]);
        }
        (FILE_SELECTION, ContextValue::FileSelection(paths)) => {
            if paths.len() > MAX_RESOURCES {
                return Ok(empty(WebInputReason::TooLarge));
            }
            // Metadata is deliberately not stat'ed here. Selection items may be files or
            // directories; unknown is accurate until a separately authorized Host operation.
            return resources(source, paths.iter().map(|p| (p, WebResourceKind::Unknown)));
        }
        (kind, ContextValue::Extension(text))
            if kind.starts_with("plugin.") && text.len() <= MAX_TEXT_BYTES =>
        {
            WebInput::Data {
                type_id: kind.into(),
                value: serde_json::from_str(text)?,
            }
        }
        _ => return Ok(empty(WebInputReason::UnsupportedType)),
    };
    Ok((input, BTreeMap::new()))
}

fn resources<'a>(
    source: ContextSource,
    paths: impl IntoIterator<Item = (&'a PathBuf, WebResourceKind)>,
) -> Result<(WebInput, Resources)> {
    let mut resources = BTreeMap::new();
    let mut items = vec![];
    for (path, kind) in paths {
        let handle = super::token()?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Selected item".into());
        if name.len() > 1024 {
            return Ok((
                WebInput::Empty {
                    reason: WebInputReason::TooLarge,
                },
                BTreeMap::new(),
            ));
        }
        items.push(WebResource {
            handle: handle.clone(),
            name,
            kind,
        });
        resources.insert(handle, path.clone());
    }
    Ok((WebInput::Resources { source, items }, resources))
}
