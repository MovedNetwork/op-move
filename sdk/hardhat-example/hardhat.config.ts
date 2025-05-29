import '@nomicfoundation/hardhat-toolbox';
import 'hardhat-umi';
import { HardhatUserConfig } from 'hardhat/config';

require('dotenv').config();

const config: HardhatUserConfig = {
    defaultNetwork: 'l2',
    solidity: '0.8.24',
    networks: {
        l2: {
            url: process.env.L2_RPC_URL,
            accounts: [process.env.PRIVATE_KEY || ''],
        },
        l1: {
            url: process.env.L1_RPC_URL,
            accounts: [process.env.PRIVATE_KEY || ''],
        },
    },
};

export default config;
