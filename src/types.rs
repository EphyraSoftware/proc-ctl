pub type Pid = u32;
pub type Port = u16;

/// A representation of a port using a specific protocol
#[derive(Debug)]
pub enum ProtocolPort {
    /// A TCP port
    Tcp(Port),
    /// A UDP port
    Udp(Port),
}
