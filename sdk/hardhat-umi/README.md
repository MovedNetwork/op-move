# Hardhat Umi Plugin

## What

This is a hardhat plugin that adds support for the Move language.
This plugin extends the `compile` task with a sub-task to compile contracts written in Move and
generate the required artifacts for testing and deployment.

## Setting up the Plugin

Right now this plugin requires some manual steps to set up.

Step 1: Install the `aptos` executable which comes with `move` tools. Follow
the [installation instructions](https://aptos.dev/en/build/cli), for instance for macOS:

```
brew install aptos
```

Step 2: Since `hardhat-umi` is written in Typescript, it needs to be compiled:

```
yarn build
```

With these steps done, you should be able to use this plugin in `hardhat-examples`, provided
that you have already set that up. For now, if you wish to use `hardhat-umi` in another
hardhat project, you would have to add it as a dependency to `package.json` located in
the root of your project, rerun `yarn install`, and add `require("hardhat-move");` to the
top of your `hardhat.config.ts`, similar to what `hardhat-examples` had done.

## Writing Contracts in Move

Move contracts should adhere to the following directory layout

```
<hardhat project root>
    - contracts
        - MyMovePackage1
            - sources
            - Move.toml
        - MyMovePackage2
            - sources
            - Move.toml
```

Currently, exactly one contract is generated from each Move package, with the
**contract name equal to the package name**. It should be noted that this is more of
a tentative design, and we may add a finer way for the user to specify the
package/module-to-contract mapping, potentially allowing custom contract names and
defining multiple contracts in the same package.

## Development

On one terminal keep building the hardhat plugin whenever a change is saved.

```
cd sdk/hardhat-umi
yarn watch
```

On another tab reload the built plugin package and compile the contracts.
Make sure to create an `.env` file using the `.env.example` template.

```
cd sdk/hardhat-example
yarn add --force -D file:../hardhat-umi && npx hardhat compile
```
