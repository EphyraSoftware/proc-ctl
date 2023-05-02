use crate::error::ProcCtlError::ConfigurationError;
use crate::error::{ProcCtlError, ProcCtlResult};
use crate::types::{Pid, ProtocolPort};
use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use std::process::Child;

/// Find the ports used by a process
pub struct PortQuery {
    address_family_flags: AddressFamilyFlags,
    proto_flags: ProtocolFlags,
    process_id: Option<Pid>,
    min_num_ports: Option<u32>,
}

impl PortQuery {
    /// Create a new query
    pub fn new() -> Self {
        PortQuery {
            address_family_flags: AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
            proto_flags: ProtocolFlags::TCP | ProtocolFlags::UDP,
            process_id: None,
            min_num_ports: None,
        }
    }

    /// Only consider IPv4 addresses
    pub fn ip_v4_only(mut self) -> Self {
        self.address_family_flags = AddressFamilyFlags::IPV4;
        self
    }

    /// Only consider IPv6 addresses
    pub fn ip_v6_only(mut self) -> Self {
        self.address_family_flags = AddressFamilyFlags::IPV6;
        self
    }

    /// Only consider TCP ports
    pub fn tcp_only(mut self) -> Self {
        self.proto_flags = ProtocolFlags::TCP;
        self
    }

    /// Only consider UDP ports
    pub fn udp_only(mut self) -> Self {
        self.proto_flags = ProtocolFlags::UDP;
        self
    }

    /// Require at least `num_ports` ports to be bound by the matched process for the query to succeed.
    pub fn expect_min_num_ports(mut self, num_ports: u32) -> Self {
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

    /// Execute the query the query
    pub fn execute(&self) -> ProcCtlResult<Vec<ProtocolPort>> {
        let ports = list_ports_for_pid(
            self.address_family_flags,
            self.proto_flags,
            self.resolve_pid()?,
        )?;

        if let Some(num) = &self.min_num_ports {
            if ports.len() < *num as usize {
                return Err(ProcCtlError::TooFewPorts(ports, *num));
            }
        }

        Ok(ports)
    }

    /// Execute the query the query and retry until it succeeds or exhausts the configured retries
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
                if count <= 0 {
                    return Err(e);
                } else {
                    tokio::time::sleep(delay).await;
                    return self.execute_with_retry(delay, count - 1).await;
                }
            }
        }
    }

    fn resolve_pid(&self) -> ProcCtlResult<Pid> {
        if let Some(pid) = &self.process_id {
            return Ok(*pid);
        }

        Err(ConfigurationError("unable to resolve a pid".to_string()))
    }
}

fn list_ports_for_pid(
    af_flags: AddressFamilyFlags,
    proto_flags: ProtocolFlags,
    pid: Pid,
) -> ProcCtlResult<Vec<ProtocolPort>> {
    let sockets_info = get_sockets_info(af_flags, proto_flags)?;

    sockets_info
        .iter()
        .filter_map(|v| {
            if v.associated_pids.contains(&pid) {
                match &v.protocol_socket_info {
                    ProtocolSocketInfo::Tcp(si) => {
                        if si.state == TcpState::Listen {
                            Some(Ok(ProtocolPort::Tcp(si.local_port)))
                        } else {
                            None
                        }
                    }
                    ProtocolSocketInfo::Udp(si) => Some(Ok(ProtocolPort::Udp(si.local_port))),
                }
            } else {
                None
            }
        })
        .collect()
}

impl Default for PortQuery {
    fn default() -> Self {
        PortQuery::new()
    }
}
