use assert_cmd::cargo::CommandCargoExt;
use retry::delay::Fixed;
use retry::retry;
use std::process::Command;

#[cfg(target_os = "linux")]
#[test]
fn port_query() {
    let mut binder = Command::cargo_bin("port-binder").unwrap();
    let mut handle = binder.spawn().unwrap();

    let query = proc_ctl::PortQuery::new()
        .tcp_only()
        .ip_v4_only()
        .process_id(handle.id())
        .expect_min_num_ports(1);

    let ports = retry(Fixed::from_millis(100).take(10), move || query.execute()).unwrap();

    handle.kill().unwrap();

    assert_eq!(1, ports.len());
}

#[cfg(target_os = "linux")]
#[test]
fn port_query_which_expects_too_many_ports() {
    let mut binder = Command::cargo_bin("port-binder").unwrap();
    let mut handle = binder.spawn().unwrap();

    let query = proc_ctl::PortQuery::new()
        .tcp_only()
        .ip_v4_only()
        .process_id_from_child(&handle)
        .expect_min_num_ports(2);

    // Only retry once, getting no ports is still a valid test if the child program hasn't bound yet
    let result = retry(Fixed::from_millis(100).take(1), move || query.execute());

    handle.kill().unwrap();

    result.expect_err("Should have had an error about too few ports");
}

#[cfg(all(feature = "resilience", target_os = "linux"))]
#[test]
fn port_query_with_sync_retry() {
    use std::time::Duration;

    let mut binder = Command::cargo_bin("port-binder").unwrap();
    let mut handle = binder.spawn().unwrap();

    let query = proc_ctl::PortQuery::new()
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

#[cfg(all(feature = "async", target_os = "linux"))]
#[tokio::test]
async fn port_query_with_async_retry() {
    use std::time::Duration;

    let mut binder = Command::cargo_bin("port-binder").unwrap();
    let mut handle = binder.spawn().unwrap();

    let query = proc_ctl::PortQuery::new()
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

#[cfg(feature = "proc")]
#[test]
fn proc_query_by_name() {
    use proc_ctl::ProcQuery;

    let mut binder = Command::cargo_bin("waiter").unwrap();
    let mut handle = binder.spawn().unwrap();

    let query = ProcQuery::new().process_name("waiter");

    let processes = query.list_processes().unwrap();

    handle.kill().unwrap();

    println!("{:?}", processes);

    assert_eq!(1, processes.len());
}

#[cfg(feature = "proc")]
#[test]
fn proc_query_for_children() {
    use proc_ctl::ProcQuery;

    let binder = Command::cargo_bin("port-binder").unwrap();
    let port_binder_path = binder.get_program();

    let mut runner = Command::cargo_bin("proc-runner").unwrap();
    let mut handle = runner.args([port_binder_path]).spawn().unwrap();

    let query = ProcQuery::new()
        .process_id_from_child(&handle)
        .expect_min_num_children(1);

    let process_names = retry(Fixed::from_millis(100).take(10), move || {
        query
            .children()
            .map(|v| v.into_iter().map(|p| p.name).collect::<Vec<String>>())
    })
    .unwrap();

    handle.kill().unwrap();

    assert_eq!(1, process_names.len());
    assert_eq!("port-binder", process_names.first().unwrap());
}

#[cfg(all(feature = "proc", feature = "resilience"))]
#[test]
fn proc_query_for_children_with_retry() {
    use proc_ctl::ProcQuery;
    use std::time::Duration;

    let binder = Command::cargo_bin("port-binder").unwrap();
    let port_binder_path = binder.get_program();

    let mut runner = Command::cargo_bin("proc-runner").unwrap();
    let mut handle = runner.args([port_binder_path]).spawn().unwrap();

    let process_names = ProcQuery::new()
        .process_id_from_child(&handle)
        .expect_min_num_children(1)
        .children_with_retry_sync(Duration::from_millis(100), 10)
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect::<Vec<String>>();

    handle.kill().unwrap();

    assert_eq!(1, process_names.len());
    assert_eq!("port-binder", process_names.first().unwrap());
}

#[cfg(all(feature = "proc", feature = "async"))]
#[tokio::test]
async fn proc_query_for_children_async_with_retry() {
    use proc_ctl::ProcQuery;
    use std::time::Duration;

    let binder = Command::cargo_bin("port-binder").unwrap();
    let port_binder_path = binder.get_program();

    let mut runner = Command::cargo_bin("proc-runner").unwrap();
    let mut handle = runner.args([port_binder_path]).spawn().unwrap();

    let process_names = ProcQuery::new()
        .process_id_from_child(&handle)
        .expect_min_num_children(1)
        .children_with_retry(Duration::from_millis(100), 10)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect::<Vec<String>>();

    handle.kill().unwrap();

    assert_eq!(1, process_names.len());
    assert_eq!("port-binder", process_names.first().unwrap());
}
