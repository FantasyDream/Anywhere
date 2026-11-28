//! Windows process-tree ownership via Job Objects and nonblocking pipe peeks.
use anyhow::{Context, Result};
use std::{
    io::Read,
    os::windows::{io::AsRawHandle, process::CommandExt},
    process::{Child, Command},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, ERROR_BROKEN_PIPE, HANDLE, WAIT_OBJECT_0},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Pipes::PeekNamedPipe,
        Threading::{WaitForSingleObject, CREATE_NO_WINDOW},
    },
};

pub(super) struct State {
    job: Option<HANDLE>,
}

// Job object handles are kernel handles that may safely be closed or terminated from any thread.
unsafe impl Send for State {}
unsafe impl Sync for State {}

impl Drop for State {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            unsafe { CloseHandle(job) };
        }
    }
}

pub(super) fn configure(command: &mut Command) -> Result<()> {
    command.creation_flags(CREATE_NO_WINDOW);
    Ok(())
}

pub(super) fn attach(child: &Child) -> Result<State> {
    let job = unsafe { CreateJobObjectW(null(), null()) };
    if job.is_null() {
        return Err(std::io::Error::last_os_error()).context("create Windows job object");
    }
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let configured = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&info).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if configured == 0 {
        let error = std::io::Error::last_os_error();
        unsafe { CloseHandle(job) };
        return Err(error).context("configure Windows job object");
    }
    let assigned = unsafe { AssignProcessToJobObject(job, child.as_raw_handle()) };
    if assigned == 0 {
        let exited = has_exited(child).unwrap_or(false);
        unsafe { CloseHandle(job) };
        if exited {
            return Ok(State { job: None });
        }
        return Err(std::io::Error::last_os_error())
            .context("assign process to Windows job object");
    }
    Ok(State { job: Some(job) })
}

pub(super) fn has_exited(child: &Child) -> Result<bool> {
    let status = unsafe { WaitForSingleObject(child.as_raw_handle(), 0) };
    Ok(status == WAIT_OBJECT_0)
}

pub(super) fn kill_tree(child: &mut Child, state: &mut State) -> Result<()> {
    if let Some(job) = state.job {
        let terminated = unsafe { TerminateJobObject(job, 1) };
        if terminated != 0 {
            return Ok(());
        }
    }
    if !has_exited(child)? {
        let _ = child.kill();
    }
    Ok(())
}

pub(super) fn read_pipe<R: Read + AsRawHandle>(reader: &mut R, buffer: &mut [u8]) -> Result<usize> {
    if buffer.is_empty() {
        return Ok(0);
    }
    let mut available = 0u32;
    let ok = unsafe {
        PeekNamedPipe(
            reader.as_raw_handle(),
            null_mut(),
            0,
            null_mut(),
            &mut available,
            null_mut(),
        )
    };
    if ok == 0 {
        let error = unsafe { GetLastError() };
        if error == ERROR_BROKEN_PIPE {
            return Ok(0);
        }
        return Err(std::io::Error::from_raw_os_error(error as i32).into());
    }
    if available == 0 {
        return Ok(0);
    }
    let to_read = (available as usize).min(buffer.len());
    match reader.read(&mut buffer[..to_read]) {
        Ok(size) => Ok(size),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::Interrupted
            ) =>
        {
            Ok(0)
        }
        Err(error) => Err(error.into()),
    }
}
