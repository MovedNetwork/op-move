#!/bin/sh
# Entrypoint of the op-geth docker container

# -o allexport Export all defined variables for use in config.sh
set -euxo allexport
. /volume/.env
WORKDIR="/volume/packages/contracts-bedrock"
SHARED="/volume/shared"
GENESIS_FILE="${WORKDIR}/deployments/genesis.json"
ROLLUP_FILE="${WORKDIR}/deployments/rollup.json"
JWT_FILE="${WORKDIR}/deployments/jwt.txt"
L1_DEPLOYMENT="${SHARED}/1337-deploy.json"
L1_RPC_URL="http://geth:58138"
L2_ALLOCS="${WORKDIR}/state-dump-42069.json"
DEPLOY_CONFIG="${WORKDIR}/deploy-config/umi.json"
DATA_DIR="${WORKDIR}/../../datadir"
TIMEOUT_SECS=1500

wait-for-it -t "${TIMEOUT_SECS}" "$(echo ${L1_RPC_URL} | cut -c 8-)"

export DEPLOY_CONFIG_PATH="${DEPLOY_CONFIG}"

# 3. Generate deploy config file
if [ ! -f "${DEPLOY_CONFIG_PATH}" ]; then
    sleep 160
    /volume/config.sh
    cd "${WORKDIR}"
fi

# Wait for op-node to generate this file
for _ in $(seq "${TIMEOUT_SECS}"); do
    if [ -f "${L1_DEPLOYMENT}" ]; then
        break
    fi
    sleep 1
done

# 5. Generate L2 genesis state dump
if [ ! -f "${L2_ALLOCS}" ]; then
    cd "${WORKDIR}"

    DEPLOY_CONFIG_PATH="${DEPLOY_CONFIG_PATH}" \
    CONTRACT_ADDRESSES_PATH="${L1_DEPLOYMENT}" \
    forge script scripts/L2Genesis.s.sol:L2Genesis \
        --sig "runWithAllUpgrades()" \
        --non-interactive
fi

if [ ! -f "${ROLLUP_FILE}" ]; then
    # 6. Generate genesis
    op-node genesis l2 \
        --deploy-config "${DEPLOY_CONFIG}" \
        --l1-deployments "${L1_DEPLOYMENT}" \
        --l2-allocs "${L2_ALLOCS}" \
        --outfile.l2 "${GENESIS_FILE}" \
        --outfile.rollup "${ROLLUP_FILE}" \
        --l1-rpc "${L1_RPC_URL}"

    # Initialize the data dir with genesis
    op-geth init \
        --datadir "${DATA_DIR}" \
        "${GENESIS_FILE}"
fi

echo "${JWT_SECRET}" > "${JWT_FILE}"

op-geth \
    --datadir "${DATA_DIR}" \
    --http \
    --http.addr 0.0.0.0 \
    --http.port 9545 \
    --http.corsdomain '*' \
    --http.vhosts '*' \
    --http.api web3,debug,eth,txpool,net,engine \
    --ws \
    --ws.addr 0.0.0.0 \
    --ws.port 9546 \
    --ws.origins '*' \
    --ws.api debug,eth,txpool,net,engine \
    --syncmode full \
    --gcmode archive \
    --nodiscover \
    --maxpeers 0 \
    --networkid 42069 \
    --authrpc.vhosts '*' \
    --authrpc.addr 0.0.0.0 \
    --authrpc.port 9551 \
    --authrpc.jwtsecret "${JWT_FILE}" \
    --rollup.disabletxpoolgossip
