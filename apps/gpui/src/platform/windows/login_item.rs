use crate::host::LoginItemState;
use anyhow::{ensure, Context, Result};
use windows_registry::CURRENT_USER;

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "Anywhere";

pub(crate) fn apply(desired: bool) -> Result<LoginItemState> {
    if !desired {
        match CURRENT_USER
            .options()
            .write()
            .open(KEY)
            .and_then(|key| key.remove_value(NAME))
        {
            Ok(()) => {}
            Err(error) if error.code().0 == 0x8007_0002_u32 as i32 => {}
            Err(error) => return Err(error.into()),
        }
        return Ok(LoginItemState::NotRegistered);
    }
    let executable = std::env::current_exe()?;
    let executable = executable
        .to_str()
        .context("executable path is not Unicode")?;
    let command = format!("\"{executable}\" --background");
    ensure!(
        command.encode_utf16().count() <= 260,
        "Windows startup command is too long"
    );
    CURRENT_USER.create(KEY)?.set_string(NAME, command)?;
    Ok(LoginItemState::Enabled)
}
