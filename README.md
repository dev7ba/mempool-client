Mempool-client
==============

Mempool-client is a command-line program to ask to [mempool-server](https://github.com/dev7ba/mempool-server) for Bitcoin mempool transactions, then it injects those transactions into a Bitcoin node.

The main purpose of mempool-server/client is to be able to fill a just-started Bitcoin node with transactions from another mempool, avoiding the time necessary for a node to 'sync with the mempool'. Be aware that there is not such a thing as a global mempool, so differences between nodes are expected.

The reasons you want your bitcoin node with a "full mempool" are varied, but for a regular user, the main reason is that you want to be able to estimate fees by yourself (i.e. you are using Sparrow wallet). Also, if you are a miner using Stratum v2 having a "good" mempool it's a must. Other reasons includes managing a webpage like https://mempoolexplorer.com or https://mempool.space. Also, you can bragg about how much transactions your mempool have (but be aware of [this](https://bitcoin.stackexchange.com/questions/118137/how-does-it-contribute-to-the-bitcoin-network-when-i-run-a-node-with-a-bigger-th)).

How does it works?
------------------

Mempool-client asks Mempool-server for mempool transactions ordered by dependency depth and arrival time. This order prevents transactions from being rejected because their parents are not found in the mempool. Then all transactions are orderly sent to a bitcoin node via `sendrawtransaction` RPC. Async streams are used to send transactions to the bitcoin node while they are beeing received from the server.

Mempool-server thas two endpoints: `/mempool/txsdata` and `/mempool/txsdatafrom/{mempool_counter}`. First endpoint downloads the whole mempool up to the current moment of the query. To signal that moment, the last mempool counter is returned along with all mempool data. The second endpoint works the same, but it returns all mempool data from a mempool counter. Mempool-client calls repeatedly at the second function until the mempool counter received is equal to the asked for. This guarantees that server and client mempool are syncronized at that point (almost, due to tx collision between already-in-node transactions).

Be aware that all transactions sent via `sendrawtransaction` to bitcoind will be sent unconditionally to all your bitcoind peers. It's recommended to execute `mempool_client` only when bitcoind has just started.

Mempool-client connects to Bitcoin RPC using user and password (deprecated), or using cookie authentication (default).

![diagram](./resources/diagram.png)

Usage
-----

If you are executing mempool-client in the same computer as bitcoind (like Sparrow Wallet), then you don't need any configuration: execute ``./mempool-client`` and wait "the" mempool to be transfered.

If your execute mempool-client from other computer connected to bitcoind node via local network then you have to configure two things: 

First, you must have a `config.toml` file in the same directory as your executable with contents like the following:

```
[bitcoindclient]
  # Use cookie authentication
  # cookieauthpath = "/home/my_linux_user/.bitcoin/.cookie"
  # If you use user/password authentication uncomment these lines
  user = "my_user"
  passwd = "my_password"
  # Bitcoin node ipaddr for rpc interface
  ipaddr = "bitcoind_ipaddr"
```
Second, you must configure ~/.bitcoin/bitcoin.conf to have the same user and password as before. You also need to specify bitcoind and mempool-client computer network ip addresses.
```
rpcuser=my_user
rpcpassword=my_password

[main]
rpcbind=127.0.0.1
rpcbind=bitcoind_ipaddr
rpcallowip=127.0.0.1
# Allows access to bitcoind RPC anywhere from your local network using the provided user/password.
rpcallowip=192.168.0.0/16 
```
This is a configuration similar to [Sparrow Wallet](https://sparrowwallet.com/docs/connect-node.html#remote-setup) but you don't need `server=1` to be enabled.

Compilling instructions
-----------------------

- Install [rust](https://rustup.rs/) in your system
- Clone the repository in a directory: `git clone https://github.com/dev7ba/mempool-client.git`
- Go into mempool-client directory and execute `cargo build` or `cargo build --release`. The executable will appear in `/mempool-client/target/debug` or in `/mempool-client/target/release`
- Enjoy
```
