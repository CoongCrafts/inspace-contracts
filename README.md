<br/>

<p align="center">
  <img src="https://github.com/CoongCrafts/inspaciness/assets/6867026/845fa326-7a44-4139-9bb8-a3c02874d3c4" height="100">
</p>

<h1 align="center">
InSpace Ink! Contracts
</h1>

<p align="center">
<a href="https://github.com/CoongCrafts/inspaciness">InSpace UI Repo</a> • <a href="https://inspace.ink">InSpace Application</a>
<p>


## Start contracts node

```shell
# MacOS
./bin/substrate-contracts-node --base-path ./db

# Linux
./bin/substrate-contracts-node-linux --base-path ./db
```

### Prerequisites & Installation

- rustc 1.76.0-nightly
- cargo 3.2.0
```shell
rustup toolchain install nightly-2023-11-20
rustup default nightly-2023-11-20-aarch64-apple-darwin
rustup component add rust-src --toolchain nightly-2023-11-20-aarch64-apple-darwin

cargo install --force --locked cargo-contract@=3.2.0
```
