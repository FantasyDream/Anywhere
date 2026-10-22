//! Bridge shortcuts adapter; exported names remain in the api facade.
use super::*;

pub(super) async fn supported_shortcut_actions(action_ids: Vec<String>) -> Result<Vec<String>> {
    application()?.supported_shortcut_actions(&action_ids)
}

pub(super) async fn shortcut_actions() -> Result<Vec<ShortcutActionDto>> {
    Ok(application()?
        .shortcut_actions()?
        .into_iter()
        .map(|action| ShortcutActionDto {
            action_id: action.action_id,
            title: action.title,
            localized_titles: action.localized_titles.into_iter().collect(),
            context_types: action.context_types,
        })
        .collect())
}

pub(super) async fn resolve_action_binding(
    session_id: u32,
    revision: u32,
    action_id: String,
) -> Result<Vec<CandidateDto>> {
    Ok(application()?
        .resolve_action_binding(session_id, revision, &action_id)?
        .into_iter()
        .map(candidate_dto)
        .collect())
}

pub(super) async fn wait_action_binding(
    session_id: u32,
    revision: u32,
    action_id: String,
) -> Result<BindingResolutionDto> {
    let resolved = application()?
        .wait_action_binding(session_id, revision, &action_id)
        .await?;
    Ok(BindingResolutionDto {
        state: resolved.state.into(),
        candidates: resolved.candidates.into_iter().map(candidate_dto).collect(),
        requires_choice: resolved.requires_choice,
    })
}

pub(super) fn shortcut_applications() -> Result<Vec<ShortcutApplicationDto>> {
    Ok(application()?
        .shortcut_applications()?
        .into_iter()
        .map(|app| ShortcutApplicationDto {
            bundle_id: app.bundle_id,
            title: app.title,
            localized_titles: app.localized_titles.into_iter().collect(),
        })
        .collect())
}

pub(super) fn resolve_shortcut_intent(
    session_id: u32,
    revision: u32,
    routes: Vec<ShortcutRouteDto>,
) -> Result<BindingResolutionDto> {
    let routes: Vec<_> = routes.into_iter().map(Into::into).collect();
    let resolved = application()?.resolve_shortcut_intent(session_id, revision, &routes)?;
    Ok(binding_resolution_dto(resolved))
}

pub(super) async fn wait_shortcut_intent(
    session_id: u32,
    revision: u32,
    routes: Vec<ShortcutRouteDto>,
) -> Result<BindingResolutionDto> {
    let routes: Vec<_> = routes.into_iter().map(Into::into).collect();
    Ok(binding_resolution_dto(
        application()?
            .wait_shortcut_intent(session_id, revision, &routes)
            .await?,
    ))
}
