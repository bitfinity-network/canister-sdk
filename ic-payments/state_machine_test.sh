#!/bin/sh

# Example script to run the state_machine tests. To make it work, replace
# the location of google protobuf repo in your system. You can find the
# needed repo at https://github.com/protocolbuffers/protobuf/tree/main/src/google/protobuf

./build_payments_canister.sh

# Run the test
cargo +nightly test -p ic-payments --features state-machine
