//! Semantic read model for the Launcher; graph identities remain in Core.
use crate::core::context::{navigation::RootIndex, ContextSource, ContextValue, ResolvedContext};
use crate::core::search::candidate::Candidate;
use anywhere_protocol::{AvailabilityState, ContextPresentation, TEXT};
fn priority(source: ContextSource) -> u8 {
    match source {
        ContextSource::FinderSelection => 0,
        ContextSource::SelectedText => 1,
        ContextSource::BrowserCurrentPage => 2,
        ContextSource::ApplicationContext => 3,
        ContextSource::ForegroundApplication => 4,
        ContextSource::Clipboard => 5,
        ContextSource::UserInput => 6,
    }
}

pub fn build(contexts: &[ResolvedContext], offers: &[Candidate]) -> Vec<ContextPresentation> {
    let roots = RootIndex::new(contexts);
    let base: Vec<_> = contexts.iter().map(ResolvedContext::presentation).collect();
    let mut children_by_root = std::collections::HashMap::<usize, Vec<usize>>::new();
    for index in 0..contexts.len() {
        children_by_root
            .entry(roots.root(index))
            .or_default()
            .push(index);
    }
    let mut offers_by_root = std::collections::HashMap::<&str, Vec<&Candidate>>::new();
    for offer in offers {
        for key in &offer.presentation.context_keys {
            offers_by_root.entry(key).or_default().push(offer);
        }
    }
    let mut result = Vec::with_capacity(contexts.len());
    for (index, context) in contexts.iter().enumerate() {
        let mut presentation = base[index].clone();
        presentation.root_key = roots.root_key(index).to_owned();
        presentation.icon_key = match &context.value {
            ContextValue::Application { .. } => "application",
            ContextValue::Directory(_) | ContextValue::Repository(_) => "folder",
            ContextValue::Project { .. } => "code",
            ContextValue::File(_) | ContextValue::FileSelection(_) => "file",
            ContextValue::Url(_) => "globe",
            _ => "text",
        }
        .into();
        presentation.label = if context.source == ContextSource::Clipboard {
            // Keep the existing clipboard redaction in the graph's public view,
            // including derived hosts, filenames and repository capabilities.
            if context.type_id == TEXT {
                "Clipboard".into()
            } else {
                presentation.label
            }
        } else {
            match &context.value {
                ContextValue::Directory(path)
                | ContextValue::File(path)
                | ContextValue::Repository(path)
                | ContextValue::Project {
                    directory: path, ..
                } => path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()),
                ContextValue::FileSelection(paths) => format!("{} selected items", paths.len()),
                ContextValue::Url(url) => url.host_str().unwrap_or("Current page").into(),
                ContextValue::Text(_) => match context.source {
                    ContextSource::SelectedText => "Selected text",
                    ContextSource::Clipboard => "Clipboard",
                    _ => "Input",
                }
                .into(),
                _ => presentation.label,
            }
        };
        if presentation.key == presentation.root_key {
            let children = children_by_root
                .get(&index)
                .map(Vec::as_slice)
                .unwrap_or_default();
            for &child_index in children {
                let child = &contexts[child_index];
                if roots.key(child_index) == presentation.key {
                    continue;
                }
                let capability = match &child.value {
                    _ if child.source == ContextSource::Clipboard => {
                        base[child_index].label.clone()
                    }
                    ContextValue::Repository(path) => format!(
                        "Git · {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    _ => base[child_index].label.clone(),
                };
                if !presentation.capabilities.contains(&capability) {
                    presentation.capabilities.push(capability);
                }
            }
            // A normal query is not an explicit new object. Recognized input is selectable.
            let selectable = context.source != ContextSource::UserInput
                || children
                    .iter()
                    .any(|&child| contexts[child].type_id != TEXT);
            if selectable {
                for offer in offers_by_root
                    .get(presentation.key.as_str())
                    .into_iter()
                    .flatten()
                {
                    presentation
                        .candidate_ids
                        .push(offer.presentation.id.clone());
                    if presentation.primary_candidate_id.is_none()
                        && offer.presentation.primary_action
                        && offer.presentation.availability.state == AvailabilityState::Enabled
                    {
                        presentation.primary_candidate_id = Some(offer.presentation.id.clone());
                    }
                }
            }
        }
        result.push(presentation);
    }
    result.sort_by(|a, b| {
        priority(a.source)
            .cmp(&priority(b.source))
            .then_with(|| a.key.cmp(&b.key))
    });
    result
}

pub fn preferred(summary: &[ContextPresentation]) -> Option<String> {
    summary
        .iter()
        .find(|context| {
            context.key == context.root_key
                && context.source != ContextSource::UserInput
                && !context.candidate_ids.is_empty()
        })
        .map(|context| context.key.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::navigation::root_key;
    use anywhere_protocol::{DIRECTORY, GIT_REPOSITORY};

    #[test]
    fn root_identity_survives_row_renumbering_and_git_keeps_its_directory() {
        let mut directory = ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory("/project/build".into()),
            ContextSource::FinderSelection,
            "test",
        );
        directory.id = "c4".into();
        let mut git = ResolvedContext::new(
            GIT_REPOSITORY,
            ContextValue::Repository("/project".into()),
            ContextSource::FinderSelection,
            "test",
        );
        git.id = "c5".into();
        git.source_context_id = Some(directory.id.clone());
        let before = root_key(&git, &[directory.clone(), git.clone()]);
        directory.id = "c8".into();
        git.source_context_id = Some(directory.id.clone());
        assert_eq!(before, root_key(&git, &[directory.clone(), git.clone()]));
        let summary = build(&[directory, git], &[]);
        assert_eq!(
            summary
                .iter()
                .find(|item| item.key == before)
                .unwrap()
                .label,
            "build"
        );
    }

    #[test]
    fn clipboard_labels_and_capabilities_do_not_reveal_derived_content() {
        let mut clipboard = ResolvedContext::new(
            TEXT,
            ContextValue::Text("/private/sensitive-project".into()),
            ContextSource::Clipboard,
            "test",
        );
        clipboard.id = "c0".into();
        let mut git = ResolvedContext::new(
            GIT_REPOSITORY,
            ContextValue::Repository("/private/sensitive-project".into()),
            ContextSource::Clipboard,
            "test",
        );
        git.id = "c1".into();
        git.source_context_id = Some(clipboard.id.clone());
        let summary = build(&[clipboard, git], &[]);
        assert_eq!(summary[0].label, "Clipboard");
        assert_eq!(summary[0].capabilities, ["Git Repository"]);
        assert!(!format!("{summary:?}").contains("sensitive-project"));
    }
}
