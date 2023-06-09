## proc-ctl

A helper library for querying and manipulating processes.

### Find what port a process is using

```rust no_run
use proc_ctl::PortQuery;

let query = PortQuery::new()
    .tcp_only()
    .ip_v4_only()
    .process_id(55932) // Get a process ID from somewhere
    .expect_min_num_ports(1);

query.execute().unwrap();
```

### Find the children of a given process

```rust no_run
use proc_ctl::ProcQuery;

let query = ProcQuery::new()
        .process_id(55932) // Get a process ID from somewhere
        .expect_min_num_children(1);

query.children().unwrap();
```
