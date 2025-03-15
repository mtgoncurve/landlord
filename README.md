# landlord &emsp; [![BUILD-img]][BUILD-link] [![NPM-img]][NPM-link] [![MIT-img]][MIT-link]

[BUILD-img]:  https://github.com/mtgoncurve/landlord/workflows/Build/badge.svg?branch=master
[BUILD-link]: https://github.com/mtgoncurve/landlord/actions?query=workflow%3ABuild
[NPM-img]:    https://img.shields.io/npm/v/@mtgoncurve/landlord
[NPM-link]:   https://www.npmjs.com/package/@mtgoncurve/landlord
[MIT-img]:    http://img.shields.io/badge/license-MIT-blue.svg
[MIT-link]:   https://github.com/mtgoncurve/landlord/blob/master/LICENSE

landlord is the simulation backend for [https://mtgoncurve.com](https://mtgoncurve.com)!

## What

landlord is a Rust library that simulates the mulligan and card draw process in **Magic: The Gathering**
in order to determine the probability to play cards on curve. The project uses [wasm-pack](https://github.com/rustwasm/wasm-pack),
a tool for building, optimizing, and packaging Rust-generated WebAssembly.

## Development

Run `make all` to see available development tasks.

### Updating the scryfall database

```
make card-update
make build
```

### Dependencies

```
brew install rustup
rustup-init
```

Verify `rustc` and `cargo` are available:

```
rustc --version
cargo --version
```

Install `wasm-pack`:

```
brew install wasm-pack
```

## Use with mtgoncurve.com locally

```
cd lib/pkg
yarn install
yarn link
```

In your local copy of the mtgoncurve.com repo:

```
cd /path/to/mtgoncurve.com
yarn link "@mtgoncurve/landlord"
```

and run the web app:

```
yarn
yarn run start
```

## License

[MIT](./LICENSE)
