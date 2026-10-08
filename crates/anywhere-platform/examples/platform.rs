//! Read-only by default. Explicit subcommands opt into OS side effects.
use anyhow::{bail, Result};
use anywhere_platform::{native_platform, ApplicationInfo};
use std::path::Path;

fn main() -> Result<()> {
    let platform = native_platform()?;
    let args: Vec<String> = std::env::args().collect();
    let value = args.get(2).map(String::as_str).unwrap_or("");
    match args.get(1).map(String::as_str).unwrap_or("capture") {
        "capture" => {
            let app = platform.frontmost_application()?;
            println!("frontmost={app:?}");
            if let Some(app) = app {
                println!("directory={:?}", platform.application_directory(&app)?);
            }
        }
        "finder" => println!(
            "{:?}",
            platform.application_directory(&ApplicationInfo {
                bundle_id: "com.apple.finder".into(),
                name: "Finder".into(),
                pid: 0,
                instance_id: None,
            })?
        ),
        "url" => platform.open_url(value)?,
        "file" => platform.open_file(Path::new(value))?,
        "reveal" => platform.reveal_file(Path::new(value))?,
        "directory" => platform.open_directory(Path::new(value))?,
        "terminal" => platform.open_terminal(Path::new(value))?,
        "copy" => platform.copy_text(value)?,
        _ => bail!("capture | finder | url | file | reveal | directory | terminal | copy [value]"),
    }
    Ok(())
}
