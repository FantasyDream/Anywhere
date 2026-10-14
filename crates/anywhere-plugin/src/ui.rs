//! Validation and bounded interpolation happen before crossing into presentation.
use anyhow::{ensure, Result};
use anywhere_protocol::ui::{DetailView, KeyValue};

pub const MAX_DETAIL_BYTES: usize = 128 * 1024;
const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_VALUE_BYTES: usize = 16 * 1024;
const MAX_ROWS: usize = 32;

pub fn validate_detail(view: &DetailView) -> Result<()> {
    ensure!(view.schema_version == 1, "unsupported UI schema_version");
    ensure!(
        !view.title.trim().is_empty() && view.title.len() <= 200,
        "detail title must be 1..200 bytes"
    );
    // The title/labels are literal UI labels. Only body/values are templates.
    ensure!(
        !view.title.contains("{{") && !view.title.contains("}}"),
        "detail title is literal"
    );
    ensure!(view.rows.len() <= MAX_ROWS, "detail exceeds 32 rows");
    ensure!(
        view.body.as_ref().is_some_and(|b| !b.trim().is_empty()) || !view.rows.is_empty(),
        "detail requires body or rows"
    );
    let mut bytes = view.title.len();
    if let Some(body) = &view.body {
        super::validate_template(body)?;
        ensure!(body.len() <= MAX_BODY_BYTES, "detail body exceeds 64 KiB");
        bytes += body.len();
    }
    for row in &view.rows {
        ensure!(
            !row.label.trim().is_empty() && row.label.len() <= 200,
            "detail label must be 1..200 bytes"
        );
        ensure!(
            !row.label.contains("{{") && !row.label.contains("}}"),
            "detail label is literal"
        );
        super::validate_template(&row.value)?;
        ensure!(
            row.value.len() <= MAX_VALUE_BYTES,
            "detail value exceeds 16 KiB"
        );
        bytes += row.label.len() + row.value.len();
    }
    ensure!(bytes <= MAX_DETAIL_BYTES, "detail exceeds 128 KiB");
    Ok(())
}

pub fn render_detail(view: &DetailView, context: &str) -> Result<DetailView> {
    // Revalidate direct DTO callers too; render only after the Core grant check.
    validate_detail(view)?;
    let mut remaining = MAX_DETAIL_BYTES - view.title.len();
    let body = view
        .body
        .as_ref()
        .map(|body| {
            let rendered =
                super::interpolate_bounded(body, context, false, remaining.min(MAX_BODY_BYTES))?;
            remaining -= rendered.len();
            Ok::<_, anyhow::Error>(rendered)
        })
        .transpose()?;
    let mut rows = Vec::with_capacity(view.rows.len());
    for row in &view.rows {
        ensure!(
            remaining >= row.label.len(),
            "expanded detail exceeds 128 KiB"
        );
        remaining -= row.label.len();
        let value =
            super::interpolate_bounded(&row.value, context, false, remaining.min(MAX_VALUE_BYTES))?;
        remaining -= value.len();
        rows.push(KeyValue {
            label: row.label.clone(),
            value,
        });
    }
    Ok(DetailView {
        schema_version: 1,
        title: view.title.clone(),
        body,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example() -> anywhere_protocol::PluginManifest {
        super::super::parse_manifest(include_str!(
            "../../../plugins/examples/context-detail.toml"
        ))
        .unwrap()
    }
    #[test]
    fn ui_requires_negotiated_protocol_and_explicit_capability() {
        let raw = include_str!("../../../plugins/examples/context-detail.toml");
        assert!(super::super::parse_manifest(raw).is_ok());
        for invalid in [
            raw.replace("protocol_version = 3", "protocol_version = 2"),
            raw.replace("protocol_version = 3", "protocol_version = 1"),
            raw.replace("capabilities = [\"ui.render\"]", "capabilities = []"),
            raw.replace("schema_version = 1", "schema_version = 2"),
            raw.replace("type = \"render_detail\"", "type = \"webview\""),
            raw.replace("{{context.value}}", "{{context.secret}}"),
            format!("{raw}\nwidget = \"arbitrary\""),
        ] {
            assert!(super::super::parse_manifest(&invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn bounded_render_treats_user_templates_as_data() {
        let anywhere_protocol::DeclarativeAction::RenderDetail { mut view } =
            example().commands.remove(0).action
        else {
            panic!("expected detail")
        };
        let rendered = render_detail(&view, "{{context.secret}} <script>").unwrap();
        assert_eq!(rendered.rows[0].value, "{{context.secret}} <script>");
        assert!(render_detail(&view, &"x".repeat(MAX_VALUE_BYTES + 1)).is_err());
        view.rows = vec![view.rows[0].clone(); MAX_ROWS + 1];
        assert!(validate_detail(&view).is_err());
        view.rows.truncate(10);
        // Individually valid fields must still share the total output budget.
        assert!(render_detail(&view, &"x".repeat(MAX_VALUE_BYTES)).is_err());
        for row in &mut view.rows {
            row.value = "x".repeat(MAX_VALUE_BYTES);
        }
        assert!(validate_detail(&view).is_err());
    }
}
