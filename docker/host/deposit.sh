#!/bin/sh
# Performs a deposit of `AMOUNT` Wei from `FROM_WALLET` on L1 to `FROM_WALLET` on L2 via `BRIDGE_ADDRESS`.
#
# The `AMOUNT` will then be reflected in the balance of FROM_WALLET on L1 (subtracted) and L2 (added).
#
# Supposed to be run on host while having the docker compose stack up.

set -eux
L1_RPC_URL="http://0.0.0.0:58138"
# FROM_WALLET=$(docker compose exec geth ls l1_datadir/keystore | grep -o '.\{40\}$')
FROM_WALLET=$(docker exec $(docker ps -q -f name=moved_geth) ls l1_datadir/keystore | grep -o '.\{40\}$')
# BRIDGE_ADDRESS=$(docker compose exec op-node cat packages/contracts-bedrock/deployments/1337-deploy.json | jq -r .L1StandardBridgeProxy)
BRIDGE_ADDRESS=$(docker exec $(docker ps -q -f name=moved_op-node) cat packages/contracts-bedrock/deployments/1337-deploy.json | jq -r .L1StandardBridgeProxy)
AMOUNT=1000000000000000000

NONCE=$(curl "${L1_RPC_URL}" \
    -s \
    -X POST \
    -H "Content-Type: application/json" \
    --data "$(printf '{"method":"eth_getTransactionCount","params":["0x%s", "latest"],"id":1,"jsonrpc":"2.0"}' "${FROM_WALLET}")" | jq -r .result)

TX_HASH=$(curl "${L1_RPC_URL}" \
  -s \
  -X POST \
  -H "Content-Type: application/json" \
  --data "$(printf '{"method":"eth_sendTransaction","params":[{"from":"0x%s","to":"%s","value":"0x%x","type":"0x2","nonce":"0x%x"}],"id":1,"jsonrpc":"2.0"}"' "${FROM_WALLET}" "${BRIDGE_ADDRESS}" "${AMOUNT}" "${NONCE}")" | jq -r .result)

echo "${TX_HASH}"
