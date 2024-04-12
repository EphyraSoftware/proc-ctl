#!/usr/bin/env bash

set -xue

cargo test
cargo test --no-default-features --test lib_test
cargo test --features resilience
cargo test --features async
cargo test --all-features
