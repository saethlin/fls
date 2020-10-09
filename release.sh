#!/bin/sh

set -e

curl --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile=minimal --default-toolchain=nightly
source $HOME/.cargo/env

git clone https://github.com/saethlin/fls --depth=1
cd fls

cargo build --release

ID=$(curl https://api.github.com/repos/saethlin/fls/releases \
    --header "Authorization: token ${GITHUB_TOKEN}" \
    --header "Accept: application/vnd.github.v3+json" \
    --data "{\"tag_name\": \"$(date -u -Iseconds | cut -d+ -f1 | sed -e s/:/-/g)\"}" | \
    python3 -c "import json,sys;obj=json.load(sys.stdin);print(obj[\"id\"])"
)

curl "https://uploads.github.com/repos/saethlin/fls/releases/${ID}/assets?name=fls" \
    --header "Authorization: token ${GITHUB_TOKEN}" \
    --header "Accept: application/vnd.github.v3+json" \
    --header "Content-type: application/octet-stream" \
    --data-binary @target/release/fls
