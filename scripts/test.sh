#!/usr/bin/env sh
set -e
export RUST_BACKTRACE=full

POCKET_IC_DIR="$(pwd)/target/pocket_ic_test_server"
export POCKET_IC_BIN="$POCKET_IC_DIR/pocket-ic"

download_file() {
    local url="$1"
    local output_path="$2"
    echo "Downloading $url to $output_path"
    curl -L -o "$output_path" "$url"
}

prepare_pocket_ic_server_binary() {
    echo "Preparing Pocket IC..."

    unameOut="$(uname -s)"
    case "${unameOut}" in
        Linux*)     MACHINE=linux;;
        Darwin*)    MACHINE=darwin;;
        *)          echo "Unsupported OS: ${unameOut}" && exit 1
    esac

    local ic_release=release-2023-11-08_23-01
    local pocket_ic_url=https://github.com/dfinity/ic/releases/download/$ic_release/pocket-ic-x86_64-$MACHINE.gz

    echo "Downloading Pocket IC from: $pocket_ic_url"
    mkdir -p $POCKET_IC_DIR
    download_file "$pocket_ic_url" "$POCKET_IC_DIR/pocket-ic.gz"

    echo "Decompressing Pocket IC"
    gzip -d "$POCKET_IC_DIR/pocket-ic.gz"
    chmod +x $POCKET_IC_BIN

    echo "Pocket IC binary is ready"
}

if [ ! -f $POCKET_IC_BIN ]; then
    prepare_pocket_ic_server_binary 
fi

# before testing, the build.sh script should be executed
cargo test 
cargo test --all-features