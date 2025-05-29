# Sample Hardhat Umi Project

This project demonstrates a basic Hardhat use case. It comes with a sample `counter` contract
that doesn't have any framework dependencies.

Define a testing account in an `.env` file using the format in `.env.example`.
The integration test will help run the entire blockchain and with a little extra code the testing
account can be funded with some initial L1 and L2 tokens.

Try running some of the following tasks:

```shell
npx hardhat compile
npx hardhat run scripts/deploy.ts --network l1
npx hardhat run scripts/deploy.ts --network l2
```

There is a usage example in `scripts/deploy.ts` under the deployment code. This is how it can be used from a web or
mobile app.
