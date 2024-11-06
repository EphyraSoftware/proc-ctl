use std::net::UdpSocket;

fn main() {
    let listener = UdpSocket::bind("[::1]:0").unwrap();
    let mut buf = [0; 10];
    listener.recv(&mut buf).unwrap();
    println!("Done receiving on UDP socket");
}
