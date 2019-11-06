#!/bin/sh

set -e

git clone --depth=1 https://github.com/saethlin/fls
cd fls

rustup default nightly
rustup show

cargo build --release

./target/release/fls

COMMIT_HASH=$(git log -n 1 --pretty=format:"%H" | cut -c-7)
curl -H "Authorization: token ${GITHUB_TOKEN}" -H "Accept: application/vnd.github.v3+json" --data "{\"tag_name\": \"${COMMIT_HASH}\"}" https://api.github.com/repos/saethlin/fls/releases >  /tmp/response

ID=$(python3 -c 'import json,sys;obj=json.load(sys.stdin);print(obj["id"])' < /tmp/response)

curl -H "Authorization: token ${GITHUB_TOKEN}" -H "Accept: application/vnd.github.v3+json" -H "Content-type: application/octet-stream" --data @target/release/fls "https://uploads.github.com/repos/saethlin/fls/releases/${ID}/assets?name=fls"
