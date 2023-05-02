## proc-ctl

A helper library for querying and manipulating processes.

### Find what port a process is using

```rust
let query = PortQuery::new()
    .tcp_only()
    .ip_v4_only()
    .process_id(55932) // Get a process ID from somewhere
    .expect_min_num_ports(1);
```
