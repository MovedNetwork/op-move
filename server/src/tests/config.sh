#!/usr/bin/env sh

# This script is used to generate the getting-started.json configuration file
# used in the Getting Started quickstart guide on the docs site. Avoids the
# need to have the getting-started.json committed to the repo since it's an
# invalid JSON file when not filled in, which is annoying.

set -eu

# Get the finalized block timestamp and hash
block=$(cast block finalized --rpc-url "$L1_RPC_URL")
timestamp=$(echo "$block" | awk '/^timestamp/ { print $2 }')
blockhash=$(echo "$block" | awk '/^hash/ { print $2 }')

# Generate the config file
config=$(cat << EOL
{
  "finalSystemOwner": "$ADMIN_ADDRESS",
  "superchainConfigGuardian": "$ADMIN_ADDRESS",
  "l1StartingBlockTag": "$blockhash",

  "l1ChainID": 1337,
  "l2ChainID": 42069,
  "l2BlockTime": 1,
  "l1BlockTime": 2,

  "maxSequencerDrift": 600,
  "sequencerWindowSize": 3600,
  "channelTimeout": 300,

  "p2pSequencerAddress": "$SEQUENCER_ADDRESS",
  "batchInboxAddress": "0xff00000000000000000000000000000000042069",
  "batchSenderAddress": "$BATCHER_ADDRESS",

  "l2OutputOracleSubmissionInterval": 30,
  "l2OutputOracleStartingBlockNumber": 0,
  "l2OutputOracleStartingTimestamp": $timestamp,

  "l2OutputOracleProposer": "$PROPOSER_ADDRESS",
  "l2OutputOracleChallenger": "$ADMIN_ADDRESS",

  "finalizationPeriodSeconds": 12,

  "proxyAdminOwner": "$ADMIN_ADDRESS",
  "baseFeeVaultRecipient": "$ADMIN_ADDRESS",
  "l1FeeVaultRecipient": "$ADMIN_ADDRESS",
  "sequencerFeeVaultRecipient": "$ADMIN_ADDRESS",

  "baseFeeVaultMinimumWithdrawalAmount": "0x8ac7230489e80000",
  "l1FeeVaultMinimumWithdrawalAmount": "0x8ac7230489e80000",
  "sequencerFeeVaultMinimumWithdrawalAmount": "0x8ac7230489e80000",
  "baseFeeVaultWithdrawalNetwork": 0,
  "l1FeeVaultWithdrawalNetwork": 0,
  "sequencerFeeVaultWithdrawalNetwork": 0,

  "enableGovernance": true,
  "governanceTokenSymbol": "OP",
  "governanceTokenName": "Optimism",
  "governanceTokenOwner": "$ADMIN_ADDRESS",

  "l2GenesisBlockGasLimit": "0x1c9c380",
  "l2GenesisBlockBaseFeePerGas": "0x3b9aca00",
  "eip1559Denominator": 50,
  "eip1559Elasticity": 6,

  "l2GenesisRegolithTimeOffset": "0x0",
  "systemConfigStartBlock": 0,

  "requiredProtocolVersion": "0x0000000000000000000000000000000000000000000000000000000000000000",
  "recommendedProtocolVersion": "0x0000000000000000000000000000000000000000000000000000000000000000",

  "fundDevAccounts": false,

  "faultGameAbsolutePrestate": "0x03c7ae758795765c6664a5d39bf63841c71ff191e9189522bad8ebff5d4eca98",
  "faultGameMaxDepth": 44,
  "faultGameClockExtension": 0,
  "faultGameMaxClockDuration": 600,
  "faultGameGenesisBlock": 0,
  "faultGameGenesisOutputRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
  "faultGameSplitDepth": 14,
  "faultGameWithdrawalDelay": 604800,

  "preimageOracleMinProposalSize": 1800000,
  "preimageOracleChallengePeriod": 86400,
  "proofMaturityDelaySeconds": 604800,
  "disputeGameFinalityDelaySeconds": 302400,
  "respectedGameType": 0,
  "useFaultProofs": false,



  "eip1559DenominatorCanyon": 250,
  "l2GenesisEcotoneTimeOffset": "0x0",
  "l2GenesisDeltaTimeOffset": "0x0",
  "l2GenesisCanyonTimeOffset": "0x0",

  "gasPriceOracleBaseFeeScalar": 141648,
  "gasPriceOracleBlobBaseFeeScalar": 810831
}
EOL
)

# Write the config file
echo "$config" > "$DEPLOY_CONFIG_PATH"
