# scryfall2landlord

Converts a [Scryfall bulk data JSON file](https://scryfall.com/docs/api/bulk-data) to an internal format required by landlord.

## Usage

Get the latest Oracle Cards data from Scryfall's bulk data API

```
$ curl --silent https://api.scryfall.com/bulk-data | jq --raw-output .data[0].download_uri
https://data.scryfall.io/oracle-cards/oracle-cards-20250315210406.json
```

Pull down the data and create the compressed data file to be ingested by landlord.

```console
curl "https://data.scryfall.io/oracle-cards/oracle-cards-20250315210406.json" -o "./scryfall-oracle-cards.json"
cargo run -- ./scryfall-oracle-cards.json all_cards.landlord
```
