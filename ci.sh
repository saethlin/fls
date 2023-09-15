#!/bin/bash

set -e
exec 2>&1
export TERM=xterm-256color

function group {
    echo "::group::$@"
    $@
    echo "::endgroup"
}

if [[ "$1" == "style" ]]
then
    group cargo fmt --check
    group cargo clippy -- -Dclippy::all
else
    group cargo build
    group cargo build --release
    group cargo build --release -Zbuild-std --target=x86_64-unknown-linux-gnu
fi
