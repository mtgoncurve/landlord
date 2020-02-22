# scryfall2landlord

Converts a [Scryfall bulk data JSON file](https://scryfall.com/docs/api/bulk-data) to an internal format required by landlord.

## Usage

```console
curl "https://archive.scryfall.com/json/scryfall-oracle-cards.json" -o "./scryfall-oracle-cards.json"
cargo run -- ./scryfall-oracle-cards.json ./data/all_cards.landlord
```
