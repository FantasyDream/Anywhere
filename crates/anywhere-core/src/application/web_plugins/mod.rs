//! Package/grant orchestration, typed Host APIs and finite background jobs.
//! UI stays in WebView; only headless jobs enter the bounded QuickJS worker.
//! Package bytes and disk mechanics stay in the repository.
mod application;
mod background_action;
mod background_provider;
mod background_runtime;
mod broker;
mod catalog;
mod invocation;
mod packages;
mod projection;
pub use packages::{WebPackageReview, WebPackageStatus, WebPackageStore};
mod host_state;
mod rpc_dispatch;
mod rpc_state;
mod transfer;

pub use catalog::{WebPackageApproval, WebPluginCatalog};
pub use invocation::{WebDocumentBinding, WebMessageSource, WebPluginService};

use anyhow::Result;

fn storage_namespace(plugin: &str, source: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "web.{}",
        hex::encode(Sha256::digest(format!("{plugin}\0{source}").as_bytes()))
    )
}

fn token() -> Result<String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("Web identity unavailable: {e}"))?;
    Ok(hex::encode(bytes))
}

#[cfg(test)]
mod broker_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod package_tests;

#[cfg(test)]
mod background_tests;
