use std::net::TcpListener;

fn main() {
    let listener = TcpListener::bind("[::1]:0").unwrap();
    listener.accept().unwrap();
    println!("Listener finished");
}
