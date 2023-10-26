#!/bin/sh

cd $(dirname $0)

cargo build || exit

cd test_crate

LD_LIBRARY_PATH="$(echo ~/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib)" ../target/debug/lockcheck
