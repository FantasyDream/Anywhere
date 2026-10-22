//! Developer-only package inspection. Never approves, installs, registers or executes.
use anyhow::{ensure, Context, Result};
use anywhere_plugin::web::VerifiedWebPackage;

fn main() -> Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!(
        arguments.len() == 1,
        "usage: inspect_web_package <package.awplugin>"
    );
    let file = std::fs::File::open(&arguments[0]).context("open Web package")?;
    let package = VerifiedWebPackage::read(file)?;
    let commands: Vec<_> = package.manifest().commands.iter().map(|command|
        serde_json::json!({"id":command.id, "actionId":package.action_id(&command.id)})
    ).collect();
    let bytes: u64 = package.assets().map(|asset| asset.size()).sum();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "id":package.manifest().id, "version":package.manifest().version,
            "contentDigest":package.digest(), "commands":commands,
            "assetCount":package.assets().count(), "expandedBytes":bytes,
            "approved":false, "installed":false
        }))?
    );
    Ok(())
}
