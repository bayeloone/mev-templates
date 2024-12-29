# MEV template written in Rust

You can find a MEV template written in Rust here.

The template follows a clean design pattern and is written with readability in mind. With the components introduced here, you can easily reproduce most of the MEV strategies known to people: sandwich, frontrunning, arbitrage, sniping, so on and so forth.

The template includes an example **DEX flashloan arbitrage strategy** to demonstrate how it can be used. It is a simple demonstration and will need some tweaking to make it work (mostly in regards to order size optimization and gas bidding strategy), though it will work as a real DEX arbitrage bot by doing:

- Retrieving all historical events from the blockchain (PairCreated).

- Create a triangular arbitrage path with all the pools retrieved from above.

- Perform a multicall request of "getReserve" calls to all the pools we're trading (1 ~ 3 second to retrieve >=6000 pools).

- Stream new headers, new pending transactions, events asynchronously.

- Simulate Uniswap V2 3-hop paths offline.

- Sign transactions and create bundles to send to Flashbots (also supports sending transactions to the mempool).

> (Professor Oak) *Good. So you are here.*

In this Github repository, you can use Rust to build your MEV project. By studying this project, you'll get a feel for how MEV strategies are built.

Most strategies share a common code base, and this repository is an attempt to include the basic tooling required for all level of traders to have in their pockets.

---

## How should I use this?

Running the template provided here is straightforward, however, you do need to create a .env file before you can begin:

- **HTTPS_URL**: your node endpoints
- **WSS_URL**: your node endpoints
- **CHAIN_ID**: 1 if Ethereum, 137 if Polygon
- **BLOCKNATIVE_TOKEN**: this is for the gas estimator service from Blocknative, you can create an account there and get the API key
- **PRIVATE_KEY**: your real wallet key, what you have to protect with your life
- **SIGNING_KEY**: just a key used for Flashbots reputation/identity
- **BOT_ADDRESS**: the address of your bot contract (V2ArbBot)

You can use the provided .env.example file and create an exact copy and name it .env (sample below):

```
HTTPS_URL=http://192.168.200.182:8545
WSS_URL=ws://192.168.200.182:8546
CHAIN_ID=137
BLOCKNATIVE_TOKEN=<token-here>

PRIVATE_KEY=0xb3e5dc08b18918cce982438a28877e440aafc01fef4c314b95d0609bf946585f
SIGNING_KEY=0x34f55bef77aca52be9f7506da40205f8ecd7e863fd3b465a5db9950247422caf
BOT_ADDRESS=0xEc1f2DADF368D5a20D494a2974bC19e421812017
```

---
