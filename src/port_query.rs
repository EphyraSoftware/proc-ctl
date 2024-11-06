use crate::error::{ProcCtlError, ProcCtlResult};
use crate::types::{Pid, ProtocolPort};
use std::process::Child;

/// Find the ports used by a process
#[derive(Debug)]
pub struct PortQuery {
    ipv4_addresses: bool,
    ipv6_addresses: bool,
    tcp_addresses: bool,
    udp_addresses: bool,
    process_id: Option<Pid>,
    min_num_ports: Option<usize>,
}

impl PortQuery {
    /// Create a new query
    pub fn new() -> Self {
        PortQuery {
            ipv4_addresses: true,
            ipv6_addresses: true,
            tcp_addresses: true,
            udp_addresses: true,
            process_id: None,
            min_num_ports: None,
        }
    }

    /// Only consider IPv4 addresses
    pub fn ip_v4_only(mut self) -> Self {
        self.ipv4_addresses = true;
        self.ipv6_addresses = false;
        self
    }

    /// Only consider IPv6 addresses
    pub fn ip_v6_only(mut self) -> Self {
        self.ipv4_addresses = false;
        self.ipv6_addresses = true;
        self
    }

    /// Only consider TCP ports
    pub fn tcp_only(mut self) -> Self {
        self.tcp_addresses = true;
        self.udp_addresses = false;
        self
    }

    /// Only consider UDP ports
    pub fn udp_only(mut self) -> Self {
        self.tcp_addresses = false;
        self.udp_addresses = true;
        self
    }

    /// Require at least `num_ports` ports to be bound by the matched process for the query to succeed.
    pub fn expect_min_num_ports(mut self, num_ports: usize) -> Self {
        self.min_num_ports = Some(num_ports);
        self
    }

    /// Set the process ID to match
    ///
    /// Either this function or `process_id_from_child` are required to be called before the query is usable.
    pub fn process_id(mut self, pid: Pid) -> Self {
        self.process_id = Some(pid);
        self
    }

    /// Get the process ID of a child process
    ///
    /// Either this function or `process_id` are required to be called before the query is usable.
    pub fn process_id_from_child(self, child: &Child) -> Self {
        self.process_id(child.id())
    }

    /// Execute the query
    pub fn execute(&self) -> ProcCtlResult<Vec<ProtocolPort>> {
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        let ports = list_ports_for_pid(self, crate::common::resolve_pid(self)?)?;
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        let ports = Vec::with_capacity(0);

        if let Some(num) = &self.min_num_ports {
            if ports.len() < *num {
                return Err(ProcCtlError::TooFewPorts(ports, *num));
            }
        }

        Ok(ports)
    }

    /// Execute the query and retry until it succeeds or exhausts the configured retries
    #[cfg(feature = "resilience")]
    pub fn execute_with_retry_sync(
        &self,
        delay: std::time::Duration,
        count: usize,
    ) -> ProcCtlResult<Vec<ProtocolPort>> {
        retry::retry(retry::delay::Fixed::from(delay).take(count), || {
            self.execute()
        })
        .map_err(|e| e.error)
    }

    /// Async equivalent of `execute_with_retry_sync`
    #[cfg(feature = "async")]
    #[async_recursion::async_recursion]
    pub async fn execute_with_retry(
        &self,
        delay: std::time::Duration,
        count: usize,
    ) -> ProcCtlResult<Vec<ProtocolPort>> {
        match self.execute() {
            Ok(ports) => Ok(ports),
            Err(e) => {
                if count == 0 {
                    Err(e)
                } else {
                    tokio::time::sleep(delay).await;
                    self.execute_with_retry(delay, count - 1).await
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn list_ports_for_pid(query: &PortQuery, pid: Pid) -> ProcCtlResult<Vec<ProtocolPort>> {
    let proc = procfs::process::Process::new(pid as i32)?;
    let fds = proc.fd()?;
    let socket_nodes = fds
        .filter_map(|fd| {
            if let Ok(fd) = fd {
                match fd.target {
                    procfs::process::FDTarget::Socket(inode) => Some(inode),
                    _ => None,
                }
            } else {
                None
            }
        })
        .collect::<std::collections::HashSet<_>>();

    let mut out = Vec::new();

    if query.tcp_addresses {
        let mut tcp_entries = proc.tcp()?;

        if query.ipv6_addresses {
            let tcp6_entries = proc.tcp6()?;

            tcp_entries.extend(tcp6_entries);
        }

        for entry in tcp_entries {
            if entry.state == procfs::net::TcpState::Listen && socket_nodes.contains(&entry.inode) {
                out.push(ProtocolPort::Tcp(entry.local_address.port()));
            }
        }
    }

    if query.udp_addresses {
        let mut udp_entries = proc.udp()?;

        if query.ipv6_addresses {
            let udp6_entries = proc.udp6()?;
            udp_entries.extend(udp6_entries);
        }

        for entry in udp_entries {
            if socket_nodes.contains(&entry.inode) {
                out.push(ProtocolPort::Udp(entry.local_address.port()));
            }
        }
    }

    Ok(out)
}

#[cfg(target_os = "windows")]
fn list_ports_for_pid(query: &PortQuery, pid: Pid) -> ProcCtlResult<Vec<ProtocolPort>> {
    let mut out = Vec::new();

    if query.tcp_addresses {
        if query.ipv4_addresses {
            let mut table = load_tcp_table(windows::Win32::Networking::WinSock::AF_INET)?;
            let table: &mut windows::Win32::NetworkManagement::IpHelper::MIB_TCPTABLE_OWNER_PID = unsafe {
                &mut *(table.as_mut_ptr()
                    as *mut windows::Win32::NetworkManagement::IpHelper::MIB_TCPTABLE_OWNER_PID)
            };

            for i in 0..table.dwNumEntries as usize {
                let row = unsafe { &*table.table.as_mut_ptr().add(i) };
                if row.dwOwningPid == pid {
                    out.push(ProtocolPort::Tcp(row.dwLocalPort as u16));
                }
            }
        }
        if query.ipv6_addresses {
            let mut table = load_tcp_table(windows::Win32::Networking::WinSock::AF_INET6)?;
            let table: &mut windows::Win32::NetworkManagement::IpHelper::MIB_TCP6TABLE_OWNER_PID = unsafe {
                &mut *(table.as_mut_ptr()
                    as *mut windows::Win32::NetworkManagement::IpHelper::MIB_TCP6TABLE_OWNER_PID)
            };

            for i in 0..table.dwNumEntries as usize {
                let row = unsafe { &*table.table.as_mut_ptr().add(i) };
                if row.dwOwningPid == pid {
                    out.push(ProtocolPort::Tcp(row.dwLocalPort as u16));
                }
            }
        }
    }
    if query.udp_addresses {
        if query.ipv4_addresses {
            let mut table = load_udp_table(windows::Win32::Networking::WinSock::AF_INET)?;
            let table: &mut windows::Win32::NetworkManagement::IpHelper::MIB_UDPTABLE_OWNER_PID = unsafe {
                &mut *(table.as_mut_ptr()
                    as *mut windows::Win32::NetworkManagement::IpHelper::MIB_UDPTABLE_OWNER_PID)
            };

            for i in 0..table.dwNumEntries as usize {
                let row = unsafe { &*table.table.as_mut_ptr().add(i) };
                if row.dwOwningPid == pid {
                    out.push(ProtocolPort::Tcp(row.dwLocalPort as u16));
                }
            }
        }
        if query.ipv6_addresses {
            let mut table = load_udp_table(windows::Win32::Networking::WinSock::AF_INET6)?;
            let table: &mut windows::Win32::NetworkManagement::IpHelper::MIB_UDP6TABLE_OWNER_PID = unsafe {
                &mut *(table.as_mut_ptr()
                    as *mut windows::Win32::NetworkManagement::IpHelper::MIB_UDP6TABLE_OWNER_PID)
            };

            for i in 0..table.dwNumEntries as usize {
                let row = unsafe { &*table.table.as_mut_ptr().add(i) };
                if row.dwOwningPid == pid {
                    out.push(ProtocolPort::Tcp(row.dwLocalPort as u16));
                }
            }
        }
    }

    Ok(out)
}

#[cfg(target_os = "windows")]
fn load_tcp_table(
    family: windows::Win32::Networking::WinSock::ADDRESS_FAMILY,
) -> ProcCtlResult<Vec<u8>> {
    let mut table = Vec::<u8>::with_capacity(0);
    let mut table_size: u32 = 0;
    for _ in 0..3 {
        let err_code = unsafe {
            windows::Win32::Foundation::WIN32_ERROR(
                windows::Win32::NetworkManagement::IpHelper::GetExtendedTcpTable(
                    Some(table.as_mut_ptr() as *mut _),
                    &mut table_size,
                    false,
                    family.0 as u32,
                    windows::Win32::NetworkManagement::IpHelper::TCP_TABLE_OWNER_PID_ALL,
                    0,
                ),
            )
        };

        if err_code == windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER {
            table.resize(table_size as usize, 0);
            continue;
        } else if err_code != windows::Win32::Foundation::NO_ERROR {
            return Err(ProcCtlError::ProcessError(format!(
                "Failed to get TCP table: {:?}",
                err_code
            )));
        }

        return Ok(table);
    }

    Err(ProcCtlError::ProcessError(
        "Failed to get TCP table".to_string(),
    ))
}

#[cfg(target_os = "windows")]
fn load_udp_table(
    family: windows::Win32::Networking::WinSock::ADDRESS_FAMILY,
) -> ProcCtlResult<Vec<u8>> {
    let mut table = Vec::<u8>::with_capacity(0);
    let mut table_size: u32 = 0;
    for _ in 0..3 {
        let err_code = unsafe {
            windows::Win32::Foundation::WIN32_ERROR(
                windows::Win32::NetworkManagement::IpHelper::GetExtendedUdpTable(
                    Some(table.as_mut_ptr() as *mut _),
                    &mut table_size,
                    false,
                    family.0 as u32,
                    windows::Win32::NetworkManagement::IpHelper::UDP_TABLE_OWNER_PID,
                    0,
                ),
            )
        };

        if err_code == windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER {
            table.resize(table_size as usize, 0);
            continue;
        } else if err_code != windows::Win32::Foundation::NO_ERROR {
            return Err(ProcCtlError::ProcessError(format!(
                "Failed to get UDP table: {:?}",
                err_code
            )));
        }

        return Ok(table);
    }

    Err(ProcCtlError::ProcessError(
        "Failed to get UDP table".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn list_ports_for_pid(query: &PortQuery, pid: Pid) -> ProcCtlResult<Vec<ProtocolPort>> {
    let mut out = Vec::new();

    if query.ipv4_addresses {
        if query.tcp_addresses {
            match std::process::Command::new("lsof")
                .arg("-a")
                .arg("-iTCP")
                .arg("-i4")
                .arg("-sTCP:LISTEN")
                .arg("-nP")
                .arg("-F0pn")
                .output()
            {
                Ok(output) => out.extend(
                    find_ports_v4(output.stdout.clone(), pid)
                        .into_iter()
                        .map(ProtocolPort::Tcp),
                ),
                Err(e) => return Err(ProcCtlError::ProcessError(e.to_string())),
            }
        }
        if query.udp_addresses {
            match std::process::Command::new("lsof")
                .arg("-a")
                .arg("-iUDP")
                .arg("-i4")
                .arg("-nP")
                .arg("-F0pn")
                .output()
            {
                Ok(output) => out.extend(
                    find_ports_v4(output.stdout.clone(), pid)
                        .into_iter()
                        .map(ProtocolPort::Udp),
                ),
                Err(e) => return Err(ProcCtlError::ProcessError(e.to_string())),
            }
        }
    }
    if query.ipv6_addresses {
        if query.tcp_addresses {
            match std::process::Command::new("lsof")
                .arg("-a")
                .arg("-iTCP")
                .arg("-i6")
                .arg("-sTCP:LISTEN")
                .arg("-nP")
                .arg("-F0pn")
                .output()
            {
                Ok(output) => out.extend(
                    find_ports_v6(output.stdout.clone(), pid)
                        .into_iter()
                        .map(ProtocolPort::Tcp),
                ),
                Err(e) => return Err(ProcCtlError::ProcessError(e.to_string())),
            }
        }
        if query.udp_addresses {
            match std::process::Command::new("lsof")
                .arg("-a")
                .arg("-iUDP")
                .arg("-i6")
                .arg("-nP")
                .arg("-F0pn")
                .output()
            {
                Ok(output) => out.extend(
                    find_ports_v6(output.stdout.clone(), pid)
                        .into_iter()
                        .map(ProtocolPort::Udp),
                ),
                Err(e) => return Err(ProcCtlError::ProcessError(e.to_string())),
            }
        }
    }

    Ok(out)
}

#[cfg(target_os = "macos")]
fn find_ports_v4(output: Vec<u8>, find_pid: Pid) -> Vec<u16> {
    let mut out = Vec::new();

    let mut index = 0;
    let len = output.len();
    while index < len {
        if output[index] != b'p' {
            break;
        }
        index += 1;

        let start_pid = index;
        while index < len && output[index] != 0 {
            index += 1;
        }

        let Some(pid) = String::from_utf8_lossy(&output[start_pid..index])
            .parse::<u32>()
            .ok()
        else {
            break;
        };
        index += 1; // 0
        index += 1; // NL

        loop {
            if pid == find_pid && index < len && output[index] == b'n' {
                while index < len && output[index] != b':' {
                    index += 1;
                }
                index += 1; // :

                let start_port = index;
                while index < len && output[index] != 0 {
                    index += 1;
                }

                if index >= len {
                    break;
                }

                if let Ok(port) = String::from_utf8_lossy(&output[start_port..index]).parse::<u16>()
                {
                    out.push(port);
                };
                index += 1; // 0
            } else {
                while index < len && output[index] != 0 {
                    index += 1;
                }
                index += 1; // 0
            }

            if index < len && output[index] == 10 {
                // NL
                index += 1;
            }

            if index >= len || output[index] == b'p' {
                break;
            }
        }
    }

    out
}

#[cfg(target_os = "macos")]
fn find_ports_v6(output: Vec<u8>, find_pid: Pid) -> Vec<u16> {
    let mut out = Vec::new();

    let mut index = 0;
    let len = output.len();
    while index < len {
        if output[index] != b'p' {
            break;
        }
        index += 1;

        let start_pid = index;
        while index < len && output[index] != 0 {
            index += 1;
        }

        let Ok(pid) = String::from_utf8_lossy(&output[start_pid..index]).parse::<u32>() else {
            break;
        };
        index += 1; // 0
        index += 1; // NL

        loop {
            if pid == find_pid && index < len && output[index] == b'n' {
                while index < len && output[index] != b']' {
                    index += 1;
                }
                index += 1; // ]

                if index < len && output[index] != b':' {
                    break;
                }
                index += 1;

                let start_port = index;
                while index < len && output[index] != 0 {
                    index += 1;
                }

                if index >= len {
                    break;
                }

                if let Ok(port) = String::from_utf8_lossy(&output[start_port..index]).parse::<u16>()
                {
                    out.push(port);
                };
                index += 1; // 0
            } else {
                while index < len && output[index] != 0 {
                    index += 1;
                }
                index += 1; // 0
            }

            if index < len && output[index] == 10 {
                // NL
                index += 1;
            }

            if index >= len || output[index] == b'p' {
                break;
            }
        }
    }

    out
}

#[cfg(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "macos",
    feature = "proc"
))]
impl crate::common::MaybeHasPid for PortQuery {
    fn get_pid(&self) -> Option<Pid> {
        self.process_id
    }
}

impl Default for PortQuery {
    fn default() -> Self {
        PortQuery::new()
    }
}
