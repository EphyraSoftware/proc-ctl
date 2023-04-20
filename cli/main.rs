use proc_ctl::PortQuery;

fn main() {
    let v = std::env::args().skip(1).take(1).next();

    if let Some(v) = v {
        let pid = v.parse::<u32>().unwrap();

        match PortQuery::new()
            .tcp_only()
            .ip_v4_only()
            .expect_min_num_ports(2)
            .process_id(pid)
            .execute()
        {
            Ok(ports) => {
                println!("{:?}", ports);
            }
            Err(e) => {
                eprintln!("Failed with error - {}", e);
            }
        }
    }
}
