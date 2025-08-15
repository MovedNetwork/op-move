# op-move

A Move VM execution layer for OP Stack.

# Integration testing

Make sure you have `go` installed on your system. Due to the pinned versions being based around August 2024, a version not older than 1.22 is required. Other dependencies include [foundry](http://getfoundry.sh/) for smart contract interaction and [jq](https://jqlang.github.io/jq/) being called indirectly by Optimism itself.

While inside the `op-move` folder, clone the Optimism monorepo. The repo is used to compile and deploy Optimism contracts.

```bash
git clone https://github.com/ethereum-optimism/optimism server/src/tests/optimism
```

To pull in the libraries required by Optimism, run the following inside that repo:

```bash
cd server/src/tests/optimism
make submodules
```

Make sure the Optimism binaries are built and are in the PATH, i.e. under the `go` path.

```bash
cd server/src/tests/optimism
git checkout f2e5a7a5
make op-node op-batcher op-proposer
mv op-node/bin/op-node ~/go/bin/
mv op-batcher/bin/op-batcher ~/go/bin/
mv op-proposer/bin/op-proposer ~/go/bin/
```

Build and install the Ethereum L1 runner from the [`geth` project](https://github.com/ethereum/go-ethereum).

```bash
git clone https://github.com/ethereum/go-ethereum.git
cd go-ethereum
git checkout tags/v1.14.5 # or higher
make geth
mv build/bin/geth ~/go/bin/geth
```

# Issues

### Go-Ethereum version

Make sure the `geth` version is compatible. Otherwise, the API communication could fail. The best way to match the versions is to check out a `go-ethereum` `tag` around the day of the `optimism` commit in submodule.
For instance, a compatible `geth` tag is `tags/v1.14.5` for the current `optimism` version.
To check which commit we use for Optimism:

```bash
cd server/src/tests/optimism
git branch
```

This shows the `(HEAD detached at <commit>)` and find the day the `<commit>` was pushed.

### Fault proof setup

When you run the integration test, if you notice an error about Optimism fault proof, run the following command inside the `optimism` root folder.

```bash
make cannon-prestate
```

### Stalled process

When you see a message with the address already being used, it means `geth` isn't shutdown correctly from a previous test run and most likely `geth` is still running in the background.
The integration test cannot shut this down automatically when it starts, so open up Activity Monitor or Task Manager to force any process with names `geth` or `op-*` to shut down.

### Optimism repo location

Make sure the `optimism` folder is inside the `op-move` project, at `op-move/server/src/tests/optimism`.
