all:
	@echo Available tasks:
	@echo make card-update '# Download the latest Scryfall JSON dump and generate a new data/all_cards.landlord'
	@echo make check       '# Run cargo check'
	@echo make clean       '# Run cargo clean'
	@echo make test        '# Run cargo test'
	@echo make bench       '# Run cargo bench'
	@echo make build       '# Builds wasm package in ./lib/pkg'
	@echo make publish     '# Publishes the wasm package in ./lib/pkg to npm'
	@echo make deploy      '# Builds and publishes the wasm package in ./lib/pkg to npm'

# This target will attempt to verify presence of certain dependencies,
# and fail early if anything is missing.
deps:
	@command -v cargo >/dev/null 2>&1 || { \
	  echo "Error: cargo not found in PATH."; \
	  echo "       Please install Rust (via rustup-init) or fix your PATH."; \
	  exit 1; \
	}
	@command -v rustup >/dev/null 2>&1 || { \
	  echo "Error: rustup not found in PATH."; \
	  echo "       Please install rustup (e.g. brew install rustup) or fix your PATH."; \
	  exit 1; \
	}
	@command -v wasm-pack >/dev/null 2>&1 || { \
	  echo "Error: wasm-pack not found in PATH."; \
	  echo "       Please install it (e.g. cargo install wasm-pack, brew install wasm-pack, etc.)."; \
	  exit 1; \
	}
	@if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then \
	  echo "Error: 'wasm32-unknown-unknown' target not installed."; \
	  echo "       Please run: 'rustup target add wasm32-unknown-unknown'"; \
	  exit 1; \
	fi

card-update: deps
	./bins/card-update.sh

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

build: deps
	# Copy top-level docs into lib for wasm-pack to bundle
	cp ./LICENSE   ./lib
	cp ./README.md ./lib
	wasm-pack build lib --scope=mtgoncurve --release

publish: build
	wasm-pack publish lib --access=public

deploy: build publish

.PHONY: all card-update install-wasm-pack check clean test bench build publish deploy deps
