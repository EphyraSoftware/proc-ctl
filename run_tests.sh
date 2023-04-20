#!/usr/bin/env bash

cargo test
cargo test --features resilience
cargo test --features async
cargo test --all-features
