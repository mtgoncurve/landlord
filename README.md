# landlord &emsp; [![BUILD-img]][BUILD-link] [![NPM-img]][NPM-link] [![MIT-img]][MIT-link]

[BUILD-img]:  https://github.com/mtgoncurve/landlord/workflows/Build/badge.svg?branch=master
[BUILD-link]: https://github.com/mtgoncurve/landlord/actions?query=workflow%3ABuild
[NPM-img]:    https://img.shields.io/npm/v/@mtgoncurve/landlord
[NPM-link]:   https://www.npmjs.com/package/@mtgoncurve/landlord
[MIT-img]:    http://img.shields.io/badge/license-MIT-blue.svg
[MIT-link]:   https://github.com/mtgoncurve/landlord/blob/master/LICENSE

landlord is the simulation backend for [https://mtgoncurve.com](https://mtgoncurve.com)!

## What

landlord is a Rust library that simulates the mulligan and card draw process in Magic: The Gathering in order to determine the probability to play cards on curve. The project uses ![wasm-pack](https://github.com/rustwasm/wasm-pack) to target the web.

## Dev

See the [Makefile](./Makefile) for useful development tasks.

## License

[MIT](./LICENSE)
