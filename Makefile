# Day-to-day tasks and manual performance checks for diffv.
# `make help` lists the targets.

FIXTURE        ?= target/fixture
FIXTURE_NOMIN  ?= target/fixture-nomin
BIN            := target/release/diffv
# The diffv on PATH (e.g. the last release) to compare against.
INSTALLED      ?= $(shell command -v diffv 2>/dev/null)

.DEFAULT_GOAL := help
.PHONY: help build run install test lint fmt check \
        fixture fixture-clean try try-watch try-installed try-installed-nomin

help: ## Show this help
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z_-]+:.*## / {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Release build (target/release/diffv)
	cargo build --release

run: build ## Run the release build in the current repo
	$(BIN)

install: ## Install this checkout as `diffv` (cargo install)
	cargo install --path .

test: ## Run the whole test suite
	cargo test --locked

lint: ## Clippy on everything, warnings fail
	cargo clippy --all-targets --locked -- -D warnings

fmt: ## Format the code
	cargo fmt

check: ## fmt check + lint + tests (what CI cares about)
	cargo fmt --check
	$(MAKE) lint
	$(MAKE) test

$(FIXTURE):
	python3 scripts/make_fixture.py $(FIXTURE)

$(FIXTURE_NOMIN):
	python3 scripts/make_fixture.py $(FIXTURE_NOMIN) --no-minified

fixture: $(FIXTURE) $(FIXTURE_NOMIN) ## Generate the big test repos under target/ (~160 MB)

fixture-clean: ## Remove the generated test repos
	rm -rf $(FIXTURE) $(FIXTURE_NOMIN)

try: build $(FIXTURE) ## Open this build on the big repo (3.3k files, 100k-line file, 3.6 MB line)
	$(BIN) $(FIXTURE)

try-watch: build $(FIXTURE) ## Same, in watch mode (edit files under target/fixture from another terminal)
	$(BIN) --watch $(FIXTURE)

try-installed: $(FIXTURE) ## Open the installed diffv on the big repo, for comparison (old builds hang on the minified file)
	@test -n "$(INSTALLED)" || { echo "no diffv on PATH"; exit 1; }
	$(INSTALLED) $(FIXTURE)

try-installed-nomin: $(FIXTURE_NOMIN) ## Installed diffv on the big repo without the minified file
	@test -n "$(INSTALLED)" || { echo "no diffv on PATH"; exit 1; }
	$(INSTALLED) $(FIXTURE_NOMIN)
