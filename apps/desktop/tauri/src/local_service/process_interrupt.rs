//! Interrupts a blocked stdin write without waiting for the writer's child lock.

#[cfg(unix)]
use std::io;

#[cfg(windows)]
use std::ptr;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        JobObjects::{AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject},
        Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE},
    },
};

#[derive(Debug)]
pub(crate) struct ProcessInterrupt {
    #[cfg(windows)]
    job: usize,
    #[cfg(unix)]
    process_group: libc::pid_t,
}

impl ProcessInterrupt {
    pub(crate) fn new(process_id: u32) -> Result<Self, String> {
        #[cfg(windows)]
        {
            let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
            if job.is_null() {
                return Err(
                    "The local-service process group could not be prepared for shutdown.".into(),
                );
            }
            let process =
                unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, process_id) };
            if process.is_null() {
                unsafe { CloseHandle(job) };
                return Err("The local-service process could not be prepared for shutdown.".into());
            }
            // A second job provides an independent kill handle while the shell plugin's child
            // handle is temporarily held by a blocked stdin writer.
            let assigned = unsafe { AssignProcessToJobObject(job, process) } != 0;
            unsafe { CloseHandle(process) };
            if !assigned {
                unsafe { CloseHandle(job) };
                return Err(
                    "The local-service process group could not be prepared for shutdown.".into(),
                );
            }
            Ok(Self { job: job as usize })
        }

        #[cfg(unix)]
        {
            let process_group = libc::pid_t::try_from(process_id)
                .map_err(|_| "The local-service process ID is invalid.")?;
            Ok(Self { process_group })
        }
    }

    /// Sends a platform process signal that makes a blocked pipe write return.
    pub(crate) fn interrupt(&self) -> Result<(), String> {
        #[cfg(windows)]
        {
            let job = self.job as HANDLE;
            if unsafe { TerminateJobObject(job, 1) } != 0 {
                return Ok(());
            }
            Err("The local-service process group could not be interrupted.".into())
        }

        #[cfg(unix)]
        {
            // The sidecar is launched in a process group whose ID is its PID.
            if unsafe { libc::killpg(self.process_group, libc::SIGKILL) } == 0 {
                return Ok(());
            }
            if io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            Err("The local-service process group could not be interrupted.".into())
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessInterrupt {
    fn drop(&mut self) {
        // SAFETY: this wrapper uniquely owns the job handle returned by CreateJobObjectW.
        unsafe { CloseHandle(self.job as HANDLE) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[cfg(windows)]
    struct TestJob(HANDLE);

    #[cfg(windows)]
    impl Drop for TestJob {
        fn drop(&mut self) {
            // SAFETY: the test wrapper uniquely owns this job handle.
            unsafe { CloseHandle(self.0) };
        }
    }

    #[cfg(windows)]
    fn assign_to_initial_job(process_id: u32) -> TestJob {
        let job_handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        assert!(!job_handle.is_null(), "initial test job is created");
        let job = TestJob(job_handle);
        let process = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, process_id) };
        assert!(!process.is_null(), "test process can be opened");
        // SAFETY: both handles remain open for the duration of assignment.
        let assigned = unsafe { AssignProcessToJobObject(job.0, process) } != 0;
        // SAFETY: this wrapper owns the process handle returned by OpenProcess.
        unsafe { CloseHandle(process) };
        assert!(assigned, "test process joins its initial job");
        job
    }

    #[test]
    fn process_interrupt_stops_the_owned_process_group() {
        #[cfg(windows)]
        let mut command = {
            let mut command = Command::new("ping.exe");
            command.args(["-t", "127.0.0.1"]);
            command
        };
        #[cfg(unix)]
        let mut command = {
            use std::os::unix::process::CommandExt;

            let mut command = Command::new("sh");
            command.args(["-c", "sleep 30"]).process_group(0);
            command
        };

        let mut child = command.spawn().expect("test child starts");
        #[cfg(windows)]
        let _initial_job = assign_to_initial_job(child.id());
        let interrupt = ProcessInterrupt::new(child.id()).expect("child can be interrupted");

        interrupt.interrupt().expect("process group is interrupted");
        child.wait().expect("owned process group leader is reaped");
    }
}
