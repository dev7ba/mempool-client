Mempool-client
==============

Mempool-client is a command-line program to ask to [mempool-server](https://github.com/dev7ba/mempool-server) for Bitcoin mempool transactions, then it injects that transactions into a Bitcoin node.

The main purpose of mempool-server/client is to be able to fill a just-started Bitcoin node with transactions from another mempool, avoiding the time necessary for a node to 'sync with the mempool'. Be aware that there is not such a thing as a global mempool, so differences between nodes are expected.

[diagram](./resources/diagram.png)

How does it works?
------------------

Mempool-client asks Mempool-server for mempool transactions ordered by dependency depth and arrival time. This order prevents transactions from beeing rejected because their parents are not found in the mempool. Then all transactions are orderly sent to a bitcoin node via `sendrawtransaction` RPC. Async streams are used to send transactions to the bitcoin node while they are beeing received from the server.

Mempool-server thas two endpoints: `/mempool/txsdata` and `/mempool/txsdatafrom/{mempool_counter}`. First endpoint downloads the whole mempool up to the current moment of the query. To signal that moment, the last mempool counter is returned along with all mempool data. The second endpoint works the same, but it returns all mempool data from a mempool counter. Mempool-client calls repeatedly at the second function until the mempool counter received is equal to the asked. This guarantees that server and client mempool are syncronized at that point (almost, due to tx collision between already-in-node transactions).

Mempool-client connects to Bitcoin RPC using user and password (deprecated), or using cookie authentication.

Usage
-----

First, you must have a `config.toml` file in the same directory as your executable with contents like the following:
```
[bitcoindclient]
  # Use cookie authentication
 	cookieauthpath = "/home/my_linux_user/.bitcoin/.cookie"
  # If you use user/password authentication uncomment these lines
  # user = "my_user"
  # passwd = "my_password"
  # Bitcoin node ipaddr for rpc interface
  ipaddr = "127.0.0.1"
  ```
Cookie authentication is the default for latest versions of Bitcoin core. You must have configured your `bitcoin.conf` with the values `rpcbind=my_ip` and `rpcallowip=my_ip` for the RPC interface to work.

Compilling instructions
-----------------------

- Install [rust](https://rustup.rs/) in your system
- Clone the repository in a directory: `git clone https://github.com/dev7ba/mempool-client.git`
- Go into mempool-client directory and execute `cargo build` or `cargo build --release`. The executable will appear in `/mempool-client/target/debug` or in `/mempool-client/target/release`
- Enjoy
```
