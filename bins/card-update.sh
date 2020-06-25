#!/usr/bin/env bash
set -x

DATE=$(date "+%Y-%m-%d")
ORACLE_CARDS="scryfall-oracle-cards-$DATE.json"
CI=${LANDLORD_IS_CI:-0}

#  0. Don't allow this script to run in a dirty working directory
git diff --exit-code --quiet
if [ $? -eq 1 ]; then
    # Changes
    echo "Run git stash or stage + commit the current working directory"
    exit 1
fi
#  1. Hit the bulk-data api for a list of bulk data download links
#     The oracle cards download url can be found in the JSON returned at https://api.scryfall.com/bulk-data
#     under data[0].download_uri
ORACLE_URL=$(curl 'https://api.scryfall.com/bulk-data' | python3 -c "import sys, json; print(json.load(sys.stdin)['data'][0]['download_uri'])")
#  2. Download the oracale cards
curl $ORACLE_URL -o "$ORACLE_CARDS"
#  3. Generate data/all_cards.landlord using the oracle cards
RUST_BACKTRACE=1 RUST_LOG=info cargo run --release --bin scryfall2landlord "$ORACLE_CARDS" "data/all_cards.landlord"
#  4. Was a new artifact generated? If so and this is the CI pipeline, then test it and upload the input file to S3
git diff --exit-code --quiet
if [ $? -eq 1 ] && [ "$CI" -eq 1 ]; then
    # Changes
    cargo test --all
    python3 --version
    pip3 --version
    pip3 install awscli --upgrade --user
    aws --version
    aws s3 cp "$ORACLE_CARDS" "s3://mtgoncurve-scryfall-archive/$INPUT"
    git config --local user.name "Card Update Bot"
    git config --local user.email "bot@mtgoncurve.com"
    git commit -am "Update all_cards.landlord ($ORACLE_CARDS)"
    git push origin master
fi
rm "$ORACLE_CARDS"
