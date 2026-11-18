//! Shared semantic presentation. Pages own data, actions and layout composition.
//! These helpers never paint a window background; native material owns it.
use super::{design_tokens::metrics, theme};
use gpui_kit::{px, InteractiveElement, Styled};

pub trait AppStyle: Styled + Sized {
    fn body_text(self) -> Self {
        self.text_size(px(metrics::BODY_SIZE))
            .line_height(px(metrics::BODY_SIZE * metrics::BODY_LINE_HEIGHT))
    }
    fn caption_text(self) -> Self {
        self.text_size(px(metrics::CAPTION_SIZE))
            .line_height(px(metrics::CAPTION_SIZE * metrics::CAPTION_LINE_HEIGHT))
    }
    fn label_text(self) -> Self {
        self.text_size(px(metrics::LABEL_SIZE))
            .line_height(px(metrics::LABEL_SIZE * metrics::CAPTION_LINE_HEIGHT))
    }
    fn result_surface(self, selected: bool) -> Self
    where
        Self: InteractiveElement,
    {
        let surface = self.rounded(px(metrics::RESULT_RADIUS));
        if selected {
            surface
                .bg(theme::result_surface(true))
                .border(px(metrics::DIVIDER_WIDTH))
                .border_color(theme::selection_outline())
        } else {
            surface
                .bg(theme::result_surface(false))
                .hover(|style| style.bg(theme::result_surface(true)))
        }
    }
    fn selected_surface(self) -> Self {
        self.bg(theme::selected())
            .border(px(metrics::DIVIDER_WIDTH))
            .border_color(theme::selection_outline())
    }
}
impl<T: Styled> AppStyle for T {}
