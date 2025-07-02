#!/bin/sh
# Entrypoint of the op-node docker container

# -o allexport Export all defined variables for use in config.sh
set -euxo allexport
. /volume/.env
WORKDIR="/volume/packages/contracts-bedrock"
SHARED="/volume/shared"
DEPLOY_CONFIG="${WORKDIR}/deploy-config/umi.json"
ROLLUP_FILE="${WORKDIR}/deployments/rollup.json"
JWT_FILE="${WORKDIR}/deployments/jwt.txt"
GENESIS_FILE="${WORKDIR}/deployments/genesis.json"
L1_DEPLOYMENT="${WORKDIR}/deployments/1337-deploy.json"
L1_RPC_URL="http://geth:58138"
L2_RPC_URL="http://op-move:8551"
OP_GETH_ADDR="op-geth"
OP_GETH_PORT="9551"
OP_MOVE_ADDR="op-move"
OP_MOVE_PORT="8545"
L2_ALLOCS="${WORKDIR}/state-dump-42069.json"
TIMEOUT_SECS=1500

# Remove genesis config to prevent other services using outdated genesis
if [ ! -f "${ROLLUP_FILE}" ]; then
    rm -f "${SHARED}/genesis.json"
fi

export DEPLOY_CONFIG_PATH="${DEPLOY_CONFIG}"

# 3. Generate deploy config file
if [ ! -f "${DEPLOY_CONFIG_PATH}" ]; then
    rm -f "${SHARED}/1337-deploy.json"

    sleep 160
    /volume/config.sh
    cd "${WORKDIR}"

    # 4. Deploy Optimism L1 contracts
    DEPLOYMENT_CONTEXT=umi \
    DEPLOY_CONFIG_PATH="${DEPLOY_CONFIG_PATH}" \
    IMPL_SALT=0000000000000000000000000000000000000000000000000000000000000000 \
    forge script scripts/Deploy.s.sol:Deploy \
        --private-key "${ADMIN_PRIVATE_KEY}" \
        --broadcast \
        --rpc-url "${L1_RPC_URL}" \
        --slow \
        --legacy \
        --non-interactive
fi

cp -f "${L1_DEPLOYMENT}" "${SHARED}/1337-deploy.json"

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
fi

cp -f "${GENESIS_FILE}" "${SHARED}/genesis.json"

echo "${JWT_SECRET}" > "${JWT_FILE}"

wait-for-it -t "${TIMEOUT_SECS}" "${OP_GETH_ADDR}:${OP_GETH_PORT}"
wait-for-it -t "${TIMEOUT_SECS}" "${OP_MOVE_ADDR}:${OP_MOVE_PORT}"

op-node \
    --l1.beacon.ignore \
    --l2 "${L2_RPC_URL}" \
    --l2.jwt-secret "${JWT_FILE}" \
    --sequencer.enabled \
    --sequencer.l1-confs 5 \
    --verifier.l1-confs 4 \
    --rollup.config "${ROLLUP_FILE}" \
    --rpc.addr 0.0.0.0 \
    --rpc.port 8547 \
    --p2p.disable \
    --rpc.enable-admin \
    --p2p.sequencer.key "${SEQUENCER_PRIVATE_KEY}" \
    --l1 ${L1_RPC_URL} \
    --l1.rpckind basic
