#!/bin/bash

export ETH_RPC_URL=http://lento-node-1-1:8545
export ANSIBLE_HOST_KEY_CHECKING=False

set -euxo pipefail
cd infrastructure/ansible

echo "[$(date)] setting up testnet with Ansible"

ansible-playbook --vault-password-file ./vault.key --extra-vars "magnus_download_url='https://api.github.com/repos/Magnus-Foundation/magnus/actions/artifacts/$MAGNUS_DOWNLOAD_ID/zip' magnus_sidecar_download_url='https://api.github.com/repos/Magnus-Foundation/magnus/actions/artifacts/$MAGNUS_SIDECAR_DOWNLOAD_ID/zip' magnus_force_reset=True magnus_relative_path='../../' magnus_commonware=False magnus_proposer='true'" -i benchmark-1 --limit lento-node-1-1 --tags devnet devnet.yml

sleep 1

echo "[$(date)] running benchmark"

ansible-playbook --vault-password-file ./vault.key \
    --extra-vars "{\"magnus_bench_download_url\": \"https://api.github.com/repos/Magnus-Foundation/magnus/actions/artifacts/$MAGNUS_BENCH_DOWNLOAD_ID/zip\", \"magnus_bench_node_sha\": \"$MAGNUS_BENCH_NODE_SHA\", \"magnus_bench_build_profile\": \"$MAGNUS_BENCH_BUILD_PROFILE\", \"magnus_bench_target_urls\": [\"http://lento-node-1-1:8545\"], \"magnus_bench_benchmark_mode\": \"$MAGNUS_BENCH_BENCHMARK_MODE\"}" \
    -i benchmark-1 --limit benchmark-1-bench --tags benchmark benchmark.yml

echo "[$(date)] benchmark done"

echo "[$(date)] copying logs"

mkdir report/

cp benchmark.json report/benchmark.json
scp ubuntu@lento-node-1-1:/home/ubuntu/.cache/reth/logs/4246/reth.log report/node-1.log
cat report/node-1.log | ../../scripts/parse_reth_timing_logs.sh > report/node-1-timings.csv

rm report/*.log
