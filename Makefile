all:
	@echo Available tasks:
	@echo make card-update '# Download the latest Scryfall JSON dump and generate a new data/all_cards.landlord'
	@echo make arena       '# Generate arena id to scryfall id associations'
	@echo make net-decks   '# Scrape the latest net decks from MTG Goldfish and generate a new data/net_decks.landlord'
	@echo make test        '# Run cargo test'
	@echo make clean       '# Run cargo clean'
	@echo make check       '# Run cargo check'
	@echo make build       '# Builds wasm package in ./lib/pkg'
	@echo make publish     '# Publishes the wasm package in ./lib/pkg to npm'
	@echo make deploy      '# Builds and publishes the wasm package in ./lib/pkg to npm'

card-update:
	./bins/card-update.sh

net-decks:
	RUST_LOG=info cargo run --release --bin mtggoldfish2landlord data/net_decks.landlord

arena:
	RUST_LOG=info cargo run --release --bin arena2scryfall

install-wasm-pack:
	curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

check:
	cargo check

clean:
	cargo clean

test:
	cargo test --all

bench:
	cargo bench

build:
	# Copy top-level docs into lib for wasm-pack to bundle
	cp ./LICENSE   ./lib
	cp ./README.md ./lib
	wasm-pack build lib --scope=mtgoncurve --release

publish:
	wasm-pack publish lib --access=public

deploy: build publish

.PHONY: all card-update arena net-decks install-wasm-pack check clean test bench build publish deploy
