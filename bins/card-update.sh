#!/usr/bin/env bash
set -x

DATE=$(date "+%Y-%m-%d")
FILE="scryfall-oracle-cards"
INPUT="$FILE-$DATE.json"
OUTPUT="data/all_cards.landlord"
CI=${LANDLORD_IS_CI:-0}

git diff --exit-code --quiet
if [ $? -eq 1 ]; then
    # Changes
    echo "Run git stash or stage + commit the current working directory"
    exit 1
fi

curl "https://archive.scryfall.com/json/$FILE.json" -o "$INPUT"
RUST_LOG=info RUST_BACKTRACE=full cargo run --release --bin scryfall2landlord "$INPUT" "$OUTPUT"
# Was a new artifact generated? If so, then test it and upload the input file to S3
git diff --exit-code --quiet
if [ $? -eq 1 ] && [ "$CI" -eq 1 ]; then
    # Changes
    cargo test --all
    python3 --version
    pip3 --version
    pip3 install awscli --upgrade --user
    aws --version
    aws s3 cp "$INPUT" "s3://mtgoncurve-scryfall-archive/$INPUT"
    git config --local user.name "Card Update Bot"
    git config --local user.email "bot@mtgoncurve.com"
    git commit -am "Update all_cards.landlord ($INPUT)"
    git push origin master
fi
rm "$INPUT"
