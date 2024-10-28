#[cfg(any(target_os = "linux", feature = "proc"))]
fn create_command_for_sample(name: &str) -> std::process::Command {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("release")
        .join(name);
    #[cfg(target_os = "windows")]
    let path = path.with_extension("exe");

    if !path.exists() {
        panic!(
            "{} does not exist, try running `cargo build --release --bins`",
            path.display()
        );
    }

    std::process::Command::new(path)
}

#[cfg(any(target_os = "linux", feature = "proc"))]
struct DropChild(std::process::Child);

#[cfg(any(target_os = "linux", feature = "proc"))]
impl DropChild {
    fn spawn(mut cmd: std::process::Command) -> Self {
        DropChild(cmd.spawn().expect("Failed to spawn child process"))
    }
}

#[cfg(any(target_os = "linux", feature = "proc"))]
impl Drop for DropChild {
    fn drop(&mut self) {
        self.0.kill().expect("Failed to kill child process");
    }
}

#[cfg(any(target_os = "linux", feature = "proc"))]
impl std::ops::Deref for DropChild {
    type Target = std::process::Child;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(any(target_os = "linux", feature = "proc"))]
impl std::ops::DerefMut for DropChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(target_os = "linux")]
#[test]
fn port_query() {
    use retry::delay::Fixed;

    let binder = create_command_for_sample("port-binder");
    let mut handle = DropChild::spawn(binder);

    let query = proc_ctl::PortQuery::new()
        .tcp_only()
        .ip_v4_only()
        .process_id(handle.id())
        .expect_min_num_ports(1);

    let ports = retry::retry(Fixed::from_millis(100).take(10), move || query.execute()).unwrap();

    handle.kill().unwrap();

    assert_eq!(1, ports.len());
}

#[cfg(target_os = "linux")]
#[test]
fn port_query_which_expects_too_many_ports() {
    use retry::delay::Fixed;

    let binder = create_command_for_sample("port-binder");
    let mut handle = DropChild::spawn(binder);

    let query = proc_ctl::PortQuery::new()
        .tcp_only()
        .ip_v4_only()
        .process_id_from_child(&handle)
        .expect_min_num_ports(2);

    // Only retry once, getting no ports is still a valid test if the child program hasn't bound yet
    let result = retry::retry(Fixed::from_millis(100).take(1), move || query.execute());

    handle.kill().unwrap();

    result.expect_err("Should have had an error about too few ports");
}

#[cfg(all(feature = "resilience", target_os = "linux"))]
#[test]
fn port_query_with_sync_retry() {
    use std::time::Duration;

    let binder = create_command_for_sample("port-binder");
    let mut handle = DropChild::spawn(binder);

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

    let binder = create_command_for_sample("port-binder");
    let mut handle = DropChild::spawn(binder);

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
    use std::process::Stdio;

    let mut cmd = create_command_for_sample("waiter")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .spawn()
        .unwrap();

    let query = ProcQuery::new().process_name("waiter");

    let processes = query.list_processes().unwrap();

    cmd.kill().unwrap();

    assert_eq!(1, processes.len());
}

#[cfg(feature = "proc")]
#[test]
fn proc_query_for_children() {
    use proc_ctl::ProcQuery;
    use retry::delay::Fixed;

    let binder = create_command_for_sample("port-binder");
    let port_binder_path = binder.get_program();

    let mut runner = create_command_for_sample("proc-runner");
    runner.args([port_binder_path]);
    let mut handle = DropChild::spawn(runner);

    let query = ProcQuery::new()
        .process_id_from_child(&handle)
        .expect_min_num_children(1);

    let process_names = retry::retry(Fixed::from_millis(100).take(10), move || {
        query
            .children()
            .map(|v| v.into_iter().map(|p| p.name).collect::<Vec<String>>())
    })
    .unwrap();

    handle.kill().unwrap();

    assert_eq!(1, process_names.len());

    #[cfg(target_os = "windows")]
    assert_eq!("port-binder.exe", process_names.first().unwrap());
    #[cfg(not(target_os = "windows"))]
    assert_eq!("port-binder", process_names.first().unwrap());
}

#[cfg(all(feature = "proc", feature = "resilience"))]
#[test]
fn proc_query_for_children_with_retry() {
    use proc_ctl::ProcQuery;
    use std::time::Duration;

    let binder = create_command_for_sample("port-binder");
    let port_binder_path = binder.get_program();

    let mut runner = create_command_for_sample("proc-runner");
    runner.args([port_binder_path]);
    let mut handle = DropChild::spawn(runner);

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

    #[cfg(target_os = "windows")]
    assert_eq!("port-binder.exe", process_names.first().unwrap());
    #[cfg(not(target_os = "windows"))]
    assert_eq!("port-binder", process_names.first().unwrap());
}

#[cfg(all(feature = "proc", feature = "async"))]
#[tokio::test]
async fn proc_query_for_children_async_with_retry() {
    use proc_ctl::ProcQuery;
    use std::time::Duration;

    let binder = create_command_for_sample("port-binder");
    let port_binder_path = binder.get_program();

    let mut runner = create_command_for_sample("proc-runner");
    runner.args([port_binder_path]);
    let mut handle = DropChild::spawn(runner);

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

    #[cfg(target_os = "windows")]
    assert_eq!("port-binder.exe", process_names.first().unwrap());
    #[cfg(not(target_os = "windows"))]
    assert_eq!("port-binder", process_names.first().unwrap());
}
