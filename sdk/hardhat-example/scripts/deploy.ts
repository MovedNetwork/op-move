import {AccountAddress, EntryFunction, FixedBytes, TransactionPayloadEntryFunction} from "@aptos-labs/ts-sdk";
import { ethers } from 'hardhat';
import { TransactionRequest } from "ethers";

async function main() {
    const [deployer] = await ethers.getSigners();
    console.log('Deploying contracts for the account:', deployer.address);

    const Counter = await ethers.getContractFactory('counter');
    const counter = await Counter.deploy();
    await counter.waitForDeployment();
    const moduleAddress = deployer.address.replace('0x', '0x000000000000000000000000');
    console.log(`Counter address: ${moduleAddress}::counter`);

    // Generate a signer for the address 0xb0b
    const bobAddress = AccountAddress.fromString('0x' + '0'.repeat(61) + 'b0b');
    const addressBytes = [33, 0, ...bobAddress.toUint8Array()];
    const signer = new FixedBytes(new Uint8Array(addressBytes));

    // Use the deployed module
    const entryFunction = EntryFunction.build(
      `${moduleAddress}::counter`,
      'get_count',
      [], // Use `parseTypeTag(..)` to get type arg from string
      [signer],
    );
    const transactionPayload = new TransactionPayloadEntryFunction(entryFunction);
    const payload = transactionPayload.bcsToHex();
    const request: TransactionRequest = {
        to: deployer.address,
        data: payload.toString(),
    };
    await deployer.sendTransaction(request);
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err);
    process.exit(1);
  });
