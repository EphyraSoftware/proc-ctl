use crate::common::{resolve_pid, MaybeHasPid};
use crate::{Pid, ProcCtlError, ProcCtlResult};
use std::path::PathBuf;
use std::process::Child;
use std::sync::Mutex;
use std::sync::OnceLock;
use sysinfo::{Process, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

/// Information about a process
#[derive(Debug, Clone)]
pub struct ProcInfo {
    /// The name
    pub name: String,
    /// The command used to launch the process
    pub cmd: Vec<String>,
    /// The path to the executable the process is running
    pub exe: Option<PathBuf>,
    /// The process ID
    pub pid: Pid,
    /// Parent process ID if relevant
    pub parent: Option<Pid>,
    /// Environment variables available to the process
    pub env: Vec<String>,
    /// The current working directory of the process
    pub cwd: Option<PathBuf>,
}

/// Get information about a process
#[derive(Debug)]
pub struct ProcQuery {
    process_id: Option<Pid>,
    name: Option<String>,
    min_num_children: Option<usize>,
}

impl ProcQuery {
    /// Create a new process query
    pub fn new() -> Self {
        ProcQuery {
            process_id: None,
            name: None,
            min_num_children: None,
        }
    }

    /// Set the process ID to match
    ///
    /// One of this, [ProcQuery::process_name] or [ProcQuery::process_id_from_child] must be called before the query is usable.
    pub fn process_id(mut self, pid: Pid) -> Self {
        self.process_id = Some(pid);
        self
    }

    /// Set the process name to match
    ///
    /// One of this, [ProcQuery::process_id] or [ProcQuery::process_id_from_child] must be called before the query is usable.
    pub fn process_name(mut self, name: impl AsRef<str>) -> Self {
        let name = name.as_ref().to_string();
        #[cfg(target_os = "windows")]
        let name = {
            let mut name = name;
            if !name.ends_with(".exe") {
                name.push_str(".exe");
            }
            name
        };
        self.name = Some(name);
        self
    }

    /// Get the process ID of a child process
    ///
    /// Either this function or `process_id` are required to be called before the query is usable.
    pub fn process_id_from_child(self, child: &Child) -> Self {
        self.process_id(child.id())
    }

    /// Require at least `num_children` children to have been started by the matched process for the query to succeed.
    pub fn expect_min_num_children(mut self, num_children: usize) -> Self {
        self.min_num_children = Some(num_children);
        self
    }

    /// List all processes matching the current filters.
    pub fn list_processes(&self) -> ProcCtlResult<Vec<ProcInfo>> {
        let mut sys_handle = sys_handle().lock().unwrap();
        sys_handle.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything(),
        );
        let processes = sys_handle.processes();

        let infos: Vec<ProcInfo> = processes
            .values()
            .filter(|p| {
                if let Some(pid) = self.process_id {
                    if p.pid().as_u32() != pid {
                        return false;
                    }
                }

                if let Some(name) = &self.name {
                    if p.name().to_string_lossy().as_ref() != name {
                        return false;
                    }
                }

                true
            })
            .map(|p| p.into())
            .collect();

        Ok(infos)
    }

    /// Find the children of the selected process
    pub fn children(&self) -> ProcCtlResult<Vec<ProcInfo>> {
        let pid = resolve_pid(self)?;

        let mut sys_handle = sys_handle().lock().unwrap();
        sys_handle.refresh_processes(ProcessesToUpdate::All, true);
        let processes = sys_handle.processes();
        let children: Vec<ProcInfo> = processes
            .values()
            .filter(|p| p.parent() == Some(sysinfo::Pid::from(pid as usize)))
            .map(|p| p.into())
            .collect();

        if let Some(num) = &self.min_num_children {
            if children.len() < *num {
                return Err(ProcCtlError::TooFewChildren(children.len(), *num));
            }
        }

        Ok(children)
    }

    /// Execute the query and retry until it succeeds or exhausts the configured retries
    #[cfg(feature = "resilience")]
    pub fn children_with_retry_sync(
        &self,
        delay: std::time::Duration,
        count: usize,
    ) -> ProcCtlResult<Vec<ProcInfo>> {
        retry::retry(retry::delay::Fixed::from(delay).take(count), || {
            self.children()
        })
        .map_err(|e| e.error)
    }

    /// Async equivalent of `children_with_retry_sync`
    #[cfg(feature = "async")]
    #[async_recursion::async_recursion]
    pub async fn children_with_retry(
        &self,
        delay: std::time::Duration,
        count: usize,
    ) -> ProcCtlResult<Vec<ProcInfo>> {
        match self.children() {
            Ok(infos) => Ok(infos),
            Err(e) => {
                if count == 0 {
                    Err(e)
                } else {
                    tokio::time::sleep(delay).await;
                    self.children_with_retry(delay, count - 1).await
                }
            }
        }
    }
}

fn sys_handle() -> &'static Mutex<System> {
    static SYS_HANDLE: OnceLock<Mutex<System>> = OnceLock::new();
    SYS_HANDLE.get_or_init(|| {
        let mut sys = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new()),
        );
        sys.refresh_processes(ProcessesToUpdate::All, true);

        Mutex::new(sys)
    })
}

impl From<&Process> for ProcInfo {
    fn from(value: &Process) -> Self {
        ProcInfo {
            name: value.name().to_string_lossy().to_string(),
            cmd: value
                .cmd()
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            exe: value.exe().map(|p| p.to_owned()),
            pid: value.pid().as_u32() as Pid,
            parent: value.parent().map(|p| p.as_u32() as Pid),
            env: value
                .environ()
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            cwd: value.cwd().map(|p| p.to_owned()),
        }
    }
}

impl MaybeHasPid for ProcQuery {
    fn get_pid(&self) -> Option<Pid> {
        self.process_id
    }
}

impl Default for ProcQuery {
    fn default() -> Self {
        ProcQuery::new()
    }
}
