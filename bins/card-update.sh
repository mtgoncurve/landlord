#!/usr/bin/env bash
set -x

DATE=$(date "+%Y-%m-%d")
ALL_FILE="scryfall-default-cards"
ALL_INPUT="$ALL_FILE-$DATE.json"
CI=${LANDLORD_IS_CI:-0}

git diff --exit-code --quiet
if [ $? -eq 1 ]; then
    # Changes
    echo "Run git stash or stage + commit the current working directory"
    exit 1
fi

curl "https://archive.scryfall.com/json/$ALL_FILE.json" -o "$ALL_INPUT"
RUST_BACKTRACE=1 RUST_LOG=info cargo run --release --bin scryfall2landlord "$ALL_INPUT" "data/all_cards.landlord"
# Was a new artifact generated? If so, then test it and upload the input file to S3
git diff --exit-code --quiet
if [ $? -eq 1 ] && [ "$CI" -eq 1 ]; then
    # Changes
    cargo test --all
    python3 --version
    pip3 --version
    pip3 install awscli --upgrade --user
    aws --version
    aws s3 cp "$ALL_INPUT" "s3://mtgoncurve-scryfall-archive/$ALL_INPUT"
    git config --local user.name "Card Update Bot"
    git config --local user.email "bot@mtgoncurve.com"
    git commit -am "Update all_cards.landlord ($ALL_INPUT)"
    git push origin master
fi
rm "$ALL_INPUT"
