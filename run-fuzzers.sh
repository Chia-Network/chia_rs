#!/bin/bash
set -euo pipefail

crates_with_fuzzers=$(echo crates/*/fuzz/..)
cores=$(getconf _NPROCESSORS_ONLN)

for i in $crates_with_fuzzers;
do
    pushd $i
    targets=$(cargo +nightly fuzz list)
    echo "Targets: " $targets
    for t in $targets;
    do
        nice cargo +nightly fuzz run --jobs $cores $t -- -max_total_time=1800 -timeout=10
        nice cargo +nightly fuzz cmin $t
    done
    popd
done
