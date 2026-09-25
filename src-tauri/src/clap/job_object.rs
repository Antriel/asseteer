//! Windows Job Object that owns the CLAP server's whole process tree.
//!
//! The server is launched as `uv run ... clap_server.py`, so the process we spawn is only
//! the `uv` launcher — the real server is a `python.exe` grandchild. Killing `uv` leaves
//! that grandchild running (holding the port, the GPU and locks on the uv cache), so the
//! tree is put in a Job Object and stopped as a unit.
//!
//! The job is created with KILL_ON_JOB_CLOSE, so if the app crashes or is force-killed the
//! OS closes the handle and terminates the tree as well.

use std::process::Child;
use std::time::Duration;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    Security::SECURITY_ATTRIBUTES,
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
        JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
        TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    },
};

/// A process tree rooted at one spawned child. On non-Windows platforms this only tracks
/// the child itself.
pub struct ProcessTree {
    child: Child,
    #[cfg(windows)]
    job: Option<JobHandle>,
}

impl ProcessTree {
    /// Take ownership of a freshly spawned child and put it in its own job. Processes it
    /// starts afterwards join the job automatically. Failing to create or assign the job
    /// is logged, not fatal: the child is still tracked and killed directly.
    pub fn new(child: Child) -> Self {
        #[cfg(windows)]
        {
            let job = JobHandle::new().and_then(|job| job.assign(&child).map(|()| job));
            if let Err(e) = &job {
                println!(
                    "[CLAP] Warning: could not put server in a job object: {}",
                    e
                );
            }
            Self {
                child,
                job: job.ok(),
            }
        }
        #[cfg(not(windows))]
        {
            Self { child }
        }
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// `Some` once the root child has exited (the rest of the tree may still be running).
    pub fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }

    /// Terminate every process in the tree without waiting.
    pub fn kill(&mut self) {
        #[cfg(windows)]
        if let Some(job) = &self.job {
            job.terminate();
        }
        let _ = self.child.kill();
    }

    /// Terminate every process in the tree and wait until all of them have exited, so the
    /// files they had open are released. Gives up after `timeout`.
    pub fn kill_and_wait(&mut self, timeout: Duration) -> Result<(), String> {
        self.kill();
        let _ = self.child.wait();
        #[cfg(windows)]
        if let Some(job) = &self.job {
            let deadline = std::time::Instant::now() + timeout;
            loop {
                match job.active_processes() {
                    Ok(0) => break,
                    Ok(n) if std::time::Instant::now() >= deadline => {
                        return Err(format!(
                            "{} CLAP server process(es) still running after {:?}",
                            n, timeout
                        ));
                    }
                    Ok(_) => std::thread::sleep(Duration::from_millis(50)),
                    Err(e) => return Err(e),
                }
            }
        }
        #[cfg(not(windows))]
        let _ = timeout;
        Ok(())
    }
}

#[cfg(windows)]
struct JobHandle(HANDLE);

// SAFETY: Job Object handles are kernel handles, usable from any thread.
#[cfg(windows)]
unsafe impl Send for JobHandle {}
#[cfg(windows)]
unsafe impl Sync for JobHandle {}

#[cfg(windows)]
impl JobHandle {
    fn new() -> Result<Self, String> {
        unsafe {
            let handle =
                CreateJobObjectW(std::ptr::null::<SECURITY_ATTRIBUTES>(), std::ptr::null());
            if handle.is_null() {
                return Err("CreateJobObjectW returned null".to_string());
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let ok = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if ok == 0 {
                CloseHandle(handle);
                return Err("SetInformationJobObject failed".to_string());
            }

            Ok(Self(handle))
        }
    }

    fn assign(&self, child: &Child) -> Result<(), String> {
        use std::os::windows::io::AsRawHandle;

        let proc_handle = child.as_raw_handle() as HANDLE;
        if proc_handle == INVALID_HANDLE_VALUE || proc_handle.is_null() {
            return Err("Invalid child process handle".to_string());
        }
        if unsafe { AssignProcessToJobObject(self.0, proc_handle) } == 0 {
            return Err("AssignProcessToJobObject failed".to_string());
        }
        Ok(())
    }

    fn terminate(&self) {
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }

    fn active_processes(&self) -> Result<u32, String> {
        unsafe {
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = std::mem::zeroed();
            let ok = QueryInformationJobObject(
                self.0,
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            );
            if ok == 0 {
                return Err("QueryInformationJobObject failed".to_string());
            }
            Ok(info.ActiveProcesses)
        }
    }
}

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        // KILL_ON_JOB_CLOSE: closing the last handle terminates whatever is left in the job.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    fn is_alive(pid: u32) -> bool {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH", "/FO", "CSV"])
            .output()
            .expect("tasklist");
        String::from_utf8_lossy(&out.stdout).contains(&format!("\"{}\"", pid))
    }

    fn pings_started_by(pid: u32) -> Vec<u32> {
        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "(Get-CimInstance Win32_Process -Filter \"ParentProcessId={} AND Name='PING.EXE'\").ProcessId",
                    pid
                ),
            ])
            .output()
            .expect("powershell");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect()
    }

    /// Same shape as `uv run python ...`: the spawned process starts a long-lived child of
    /// its own. Stopping the tree must take the grandchild down too — killing only the
    /// root is what left the CLAP server holding the uv cache.
    #[test]
    fn kill_and_wait_stops_grandchildren() {
        let child = Command::new("cmd")
            .args(["/C", "ping -n 60 127.0.0.1 > NUL"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn cmd");
        let mut tree = ProcessTree::new(child);
        let root = tree.id();

        let mut grandchildren = Vec::new();
        for _ in 0..50 {
            grandchildren = pings_started_by(root);
            if !grandchildren.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(!grandchildren.is_empty(), "cmd never started ping");

        tree.kill_and_wait(Duration::from_secs(10))
            .expect("tree exits");

        assert!(!is_alive(root));
        for pid in grandchildren {
            assert!(!is_alive(pid), "grandchild {} survived", pid);
        }
    }
}
