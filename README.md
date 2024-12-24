# op-move

A Move VM execution layer for OP Stack.

# Integration testing

Make sure you have `go` installed on your system. Other dependencies include [foundry](http://getfoundry.sh/) for smart contract interaction and probably [jq](https://jqlang.github.io/jq/) being called indirectly by Optimism itself.

Optimism monorepo is pulled in as a submodule of this repo. The submodule is used to compile and deploy Optimism contracts.
If you don't see it inside the test folder, try bringing in the Optimism submodule manually
```bash
git submodule update --init --recursive
```

Make sure the Optimism binaries are built and are in the PATH, i.e. under the `go` path.
```bash
cd server/src/tests/optimism
make op-node op-batcher op-proposer
mv op-node/bin/op-node ~/go/bin/
mv op-batcher/bin/op-batcher ~/go/bin/
mv op-proposer/bin/op-proposer ~/go/bin/
```

Similarly, clone the [`op-geth` project](https://github.com/ethereum-optimism/op-geth) and install the `geth` binary, but save it as `op-geth`.
```bash
git clone https://github.com/ethereum-optimism/op-geth.git
cd op-geth
make geth
mv build/bin/geth ~/go/bin/op-geth # make sure it's saved as op-geth instead of geth
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
Make sure the `geth` and `op-geth` versions are compatible. Otherwise, the API communication could fail. The best way to match the versions is to check out a `go-ethereum` `tag` around the day of the `optimism` commit in submodule.
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
