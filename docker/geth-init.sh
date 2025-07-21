#!/bin/bash
# Background tasks of the geth docker container

# -e Exit if a command fails
# -u Treat unset or undefined variables as errors
# -x Print out command arguments during execution
set -eux
. /volume/.env

# Wait for the RPC node to become available
wait-for-it "${L1_RPC_ADDR}:${L1_RPC_PORT}"

# Prefund Optimism service accounts
./prefund.sh

# Deploy Optimism factory deployer contract
cast publish --async --rpc-url "${L1_RPC_URL}" "${SIGNED_L1_CONTRACT_TX}"
