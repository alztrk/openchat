//! Windows Job Object management and resource ceiling configuration.

use std::{
    io,
    mem::size_of,
    os::windows::io::{AsRawHandle, OwnedHandle},
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::HANDLE,
    System::JobObjects::{
        CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_BASIC_LIMIT_INFORMATION,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
        JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    },
};

use super::types::{SandboxJobLimits, last_os_error, owned_handle};

/// Creates a job object that terminates its assigned process tree when its last handle closes.
///
/// Optional positive process and memory limits are applied before the job is returned.
pub(crate) fn create_kill_on_close_job(
    limits: Option<SandboxJobLimits>,
) -> io::Result<OwnedHandle> {
    // SAFETY: Null attributes and name request a new unnamed job object; Windows returns the handle.
    let handle = unsafe { CreateJobObjectW(null(), null()) };
    let job = owned_handle(handle)?;
    let Some(limits) = limits.filter(|limits| {
        limits.active_process_limit > 0
            && limits.process_memory_limit_bytes > 0
            && limits.job_memory_limit_bytes > 0
    }) else {
        if limits.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The sandbox job limits must be positive.",
            ));
        }
        return configure_kill_on_close_job(job);
    };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                | JOB_OBJECT_LIMIT_PROCESS_MEMORY
                | JOB_OBJECT_LIMIT_JOB_MEMORY,
            ActiveProcessLimit: limits.active_process_limit,
            ..Default::default()
        },
        ProcessMemoryLimit: limits.process_memory_limit_bytes,
        JobMemoryLimit: limits.job_memory_limit_bytes,
        ..Default::default()
    };
    // SAFETY: `job` is live and `limits` is initialized and sized for the selected information class.
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle() as HANDLE,
            JobObjectExtendedLimitInformation,
            (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0
    {
        return Err(last_os_error());
    }
    Ok(job)
}

/// Applies only the kill-on-close policy to an existing job object.
pub(crate) fn configure_kill_on_close_job(job: OwnedHandle) -> io::Result<OwnedHandle> {
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: `job` is live and `limits` is initialized and sized for the selected information class.
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle() as HANDLE,
            JobObjectExtendedLimitInformation,
            (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0
    {
        return Err(last_os_error());
    }
    Ok(job)
}

/// Returns the number of processes still associated with a Windows job object.
pub(crate) fn job_active_process_count(job: HANDLE) -> io::Result<u32> {
    let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    // SAFETY: `job` is a live job handle and `accounting` is writable for the requested class.
    if unsafe {
        QueryInformationJobObject(
            job,
            JobObjectBasicAccountingInformation,
            (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
            size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            null_mut(),
        )
    } == 0
    {
        return Err(last_os_error());
    }
    Ok(accounting.ActiveProcesses)
}
