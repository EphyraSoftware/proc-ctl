use assert_cmd::cargo::CommandCargoExt;
use proc_ctl::PortQuery;
use retry::delay::Fixed;
use retry::retry;
use std::process::Command;

#[test]
fn port_query() {
    let mut binder = Command::cargo_bin("port-binder").unwrap();
    let mut handle = binder.spawn().unwrap();

    let query = PortQuery::new()
        .tcp_only()
        .ip_v4_only()
        .process_id(handle.id())
        .expect_min_num_ports(1);

    let ports = retry(Fixed::from_millis(100).take(10), move || query.execute()).unwrap();

    handle.kill().unwrap();

    assert_eq!(1, ports.len());
}

#[test]
fn port_query_which_expects_too_many_ports() {
    let mut binder = Command::cargo_bin("port-binder").unwrap();
    let mut handle = binder.spawn().unwrap();

    let query = PortQuery::new()
        .tcp_only()
        .ip_v4_only()
        .process_id_from_child(&handle)
        .expect_min_num_ports(2);

    // Only retry once, getting no ports is still a valid test if the child program hasn't bound yet
    let result = retry(Fixed::from_millis(100).take(1), move || query.execute());

    handle.kill().unwrap();

    result.expect_err("Should have had an error about too few ports");
}

#[cfg(feature = "resilience")]
#[test]
fn port_query_with_sync_retry() {
    use std::time::Duration;

    let mut binder = Command::cargo_bin("port-binder").unwrap();
    let mut handle = binder.spawn().unwrap();

    let query = PortQuery::new()
        .tcp_only()
        .ip_v4_only()
        .process_id_from_child(&handle)
        .expect_min_num_ports(1);

    let ports = query
        .execute_with_retry_sync(Duration::from_millis(100), 10)
        .unwrap();

    handle.kill().unwrap();

    assert_eq!(1, ports.len());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn port_query_with_async_retry() {
    use std::time::Duration;

    let mut binder = Command::cargo_bin("port-binder").unwrap();
    let mut handle = binder.spawn().unwrap();

    let query = PortQuery::new()
        .tcp_only()
        .ip_v4_only()
        .process_id_from_child(&handle)
        .expect_min_num_ports(1);

    let ports = query
        .execute_with_retry(Duration::from_millis(100), 10)
        .await
        .unwrap();

    handle.kill().unwrap();

    assert_eq!(1, ports.len());
}
