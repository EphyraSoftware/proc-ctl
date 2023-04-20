pub type Pid = u32;
pub type Port = u16;

#[derive(Debug)]
pub enum ProtocolPort {
    Tcp(Port),
    Udp(Port),
}
