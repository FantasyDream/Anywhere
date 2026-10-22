//! URL actions.
use crate::core::action::availability::AvailabilityRule;
use crate::core::action::bundle::{ActionBundle, ActionSpec};
use crate::core::action::handler::{ActionHandler, ExecutionOutput};
use crate::core::context::{ContextValue, ResolvedContext};
use crate::core::session::facts::CLIPBOARD;
use anyhow::{bail, Result};
use anywhere_platform::Platform;
use anywhere_protocol::*;

struct OpenBrowser;
struct CopyUrl;

impl ActionHandler for OpenBrowser {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        let ContextValue::Url(url) = &context.value else {
            bail!("URL context required");
        };
        crate::features::host_services::for_feature(super::descriptor(), platform)?
            .open_url(url.as_str())?;
        Ok(ExecutionOutput::dismiss())
    }
}

impl ActionHandler for CopyUrl {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        let ContextValue::Url(url) = &context.value else {
            bail!("URL context required");
        };
        crate::features::host_services::for_feature(super::descriptor(), platform)?
            .copy_text(url.as_str())?;
        Ok(ExecutionOutput::dismiss())
    }
}

pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: crate::features::groups::web(),
        actions: vec![
            ActionSpec {
                localized_titles: &[("zh", "在默认浏览器中打开")],
                id: "core.url.open",
                title: "Open in Default Browser",
                accepts: URL,
                handler: &OpenBrowser,
                availability: AvailabilityRule::Enabled,
                base_priority: 30,
                icon: "globe",
                aliases: &["browser", "open"],
                keywords: &["link"],
                home_eligible: true,
                primary_action: true,
            },
            ActionSpec {
                localized_titles: &[("zh", "复制网址")],
                id: "core.url.copy",
                title: "Copy URL",
                accepts: URL,
                handler: &CopyUrl,
                availability: AvailabilityRule::hide_if(CLIPBOARD),
                base_priority: 20,
                icon: "copy-url",
                aliases: &["copy", "link", "clipboard"],
                keywords: &["url"],
                home_eligible: true,
                primary_action: false,
            },
        ],
    }
}
