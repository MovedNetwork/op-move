#!/bin/bash
# Entrypoint of the geth docker container

# -e Exit if a command fails
# -u Treat unset or undefined variables as errors
# -x Print out command arguments during execution
set -eux
L1_DATADIR="./l1_datadir"

# In case of restart, data dir will not be empty
# Wipe it to avoid an attempt to restore the state which is not persisted in `--dev` mode
rm -rf "${L1_DATADIR}" && mkdir -p "${L1_DATADIR}"

./keystore.sh &
./geth-init.sh &

# Ephemeral proof-of-authority network with a pre-funded developer account,
# with automatic mining when there are pending transactions.
geth \
  --dev \
  --dev.period 3 \
  --datadir "${L1_DATADIR}" \
  --rpc.allow-unprotected-txs \
  --http \
  --http.addr 0.0.0.0 \
  --http.port 58138 \
  --http.corsdomain '*' \
  --http.api 'web3,debug,eth,txpool,net,engine' \
  --http.vhosts '*'
