//! Execute the launch candidate after the application has checked session currency.
use crate::core::action::handler::ExecutionOutput;
use anyhow::{bail, Result};
use anywhere_platform::Platform;

pub fn execute(path: &str, platform: &dyn Platform) -> Result<ExecutionOutput> {
    if !platform.launch_application(path)? {
        bail!("could not launch {path}");
    }
    Ok(ExecutionOutput::dismiss())
}
