//! Host-owned Web metadata, invocation/capability coordination and package use cases.
//! Package bytes and disk mechanics stay in the repository; no JS runs here.
mod application;
mod broker;
mod catalog;
mod invocation;
mod packages;
mod projection;
pub use packages::{WebPackageReview, WebPackageStatus, WebPackageStore};
mod rpc_dispatch;
mod rpc_state;
mod transfer;
pub use broker::{WebConfirmation, WebDispatch};

pub use catalog::{WebPackageApproval, WebPluginCatalog};
pub use invocation::{WebDocumentBinding, WebMessageSource, WebPluginService};

use anyhow::Result;

fn storage_namespace(plugin: &str, source: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "web.{:x}",
        Sha256::digest(format!("{plugin}\0{source}").as_bytes())
    )
}

fn token() -> Result<String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("Web identity unavailable: {e}"))?;
    use std::fmt::Write;
    let mut result = String::with_capacity(64);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("writing to String");
    }
    Ok(result)
}

#[cfg(test)]
mod broker_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod package_tests;
