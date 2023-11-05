#!/bin/sh

cd $(dirname $0)

cargo build || exit

PATH="$(pwd)/target/debug:$PATH"

cd test_crate

cargo lockcheck
# LD_LIBRARY_PATH="$(echo ~/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib)" cargo lockcheck
