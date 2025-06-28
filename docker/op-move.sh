#!/bin/sh
# Entrypoint of the op-move docker container

set -eux
SHARED="/volume/shared"
GENESIS_FILE="${SHARED}/genesis.json"
TIMEOUT_SECS=1500

# Wait for op-node to generate this file
for _ in $(seq "${TIMEOUT_SECS}"); do
    if [ -f "${GENESIS_FILE}" ]; then
        break
    fi
    sleep 1
done

/volume/op-move --genesis.l2-contract-genesis "${GENESIS_FILE}"
