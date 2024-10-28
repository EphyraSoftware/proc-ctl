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
        #[cfg(target_os = "linux")]
        let ports = list_ports_for_pid(self, crate::common::resolve_pid(self)?)?;
        #[cfg(not(target_os = "linux"))]
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
        let mut tcp_entries = procfs::net::tcp()?;

        if query.ipv6_addresses {
            let tcp6_entries = procfs::net::tcp6()?;
            tcp_entries.extend(tcp6_entries);
        }

        for entry in tcp_entries {
            if entry.state == procfs::net::TcpState::Listen && socket_nodes.contains(&entry.inode) {
                out.push(ProtocolPort::Tcp(entry.local_address.port()));
            }
        }
    }

    if query.udp_addresses {
        let mut udp_entries = procfs::net::udp()?;

        if query.ipv6_addresses {
            let udp6_entries = procfs::net::udp6()?;
            udp_entries.extend(udp6_entries);
        }

        for entry in udp_entries {
            if entry.state == procfs::net::UdpState::Established
                && socket_nodes.contains(&entry.inode)
            {
                out.push(ProtocolPort::Udp(entry.local_address.port()));
            }
        }
    }

    Ok(out)
}

#[cfg(any(target_os = "linux", feature = "proc"))]
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
