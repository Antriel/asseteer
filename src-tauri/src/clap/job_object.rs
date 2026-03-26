//! Windows Job Object for automatic child process cleanup.
//!
//! Creates a Job Object with KILL_ON_JOB_CLOSE so that if the parent process
//! crashes or is force-killed, the OS automatically terminates child processes.

#[cfg(windows)]
use once_cell::sync::Lazy;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    Security::SECURITY_ATTRIBUTES,
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    },
};

#[cfg(windows)]
static JOB_HANDLE: Lazy<JobHandle> = Lazy::new(|| {
    JobHandle::new().expect("Failed to create Windows Job Object for child process cleanup")
});

#[cfg(windows)]
struct JobHandle(HANDLE);

// SAFETY: The Job Object handle is thread-safe — Windows kernel objects can be
// used from any thread. We only create it once and assign processes to it.
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
}

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Assign a child process to the Job Object so it is killed when this process exits.
///
/// On non-Windows platforms this is a no-op (Unix handles this differently via
/// process groups or PDEATHSIG, but for now we only need Windows support).
#[cfg(windows)]
pub fn assign_child_to_job(child: &std::process::Child) -> Result<(), String> {
    use std::os::windows::io::AsRawHandle;

    let job = &JOB_HANDLE.0;
    let proc_handle = child.as_raw_handle() as HANDLE;

    if proc_handle == INVALID_HANDLE_VALUE || proc_handle.is_null() {
        return Err("Invalid child process handle".to_string());
    }

    let ok = unsafe { AssignProcessToJobObject(*job, proc_handle) };
    if ok == 0 {
        Err("AssignProcessToJobObject failed".to_string())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn assign_child_to_job(_child: &std::process::Child) -> Result<(), String> {
    // No-op on non-Windows platforms
    Ok(())
}
