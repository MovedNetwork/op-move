#!/bin/sh
# Reads a balance of Wei on `FROM_WALLET` from L2.
#
# Supposed to be run on host while having the docker compose stack up.

set -eux
L2_RPC_URL="http://0.0.0.0:8545"
# FROM_WALLET=$(docker compose exec geth ls l1_datadir/keystore | grep -o '.\{40\}$')
FROM_WALLET=$(docker exec $(docker ps -q -f name=moved_geth) ls l1_datadir/keystore | grep -o '.\{40\}$')

BALANCE=$(curl "${L2_RPC_URL}" \
    -s \
    -X POST \
    -H "Content-Type: application/json" \
    --data "$(printf '{"method":"eth_getBalance","params":["0x%s", "latest"],"id":1,"jsonrpc":"2.0"}' "${FROM_WALLET}")" | jq -r .result)

echo $((BALANCE))
