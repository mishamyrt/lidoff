# lidoff - MacBook lid angle brightness daemon
VERSION = 0.4.0

.PHONY: clean
clean: ## clean build artifacts
	cargo clean

.PHONY: fmt
fmt: ## run cargo fmt and clang-format
	@cargo fmt --all
	@find \
		crates/ \
		-iname '*.h' -o -iname '*.c' -o -iname '*.m' \
		| xargs clang-format -i

.PHONY: lint
lint: ## run cargo clippy and clang-tidy
	cargo clippy --workspace --all-targets

.PHONY: test
test: ## run Rust tests
	cargo test --workspace

.PHONY: check
check: ## run formatting, lint, tests, and static analysis
	$(MAKE) fmt
	$(MAKE) lint
	$(MAKE) test

.PHONY: publish
publish: ## publish the daemon
	git tag "v$(VERSION)"
	git-cliff -o CHANGELOG.md
	git tag -d "v$(VERSION)"
	git add Makefile CHANGELOG.md
	git commit -m "chore: release v$(VERSION)"
	git tag "v$(VERSION)"
	git push
	git push --tags

.PHONY: help
help: ## print this message
	@echo "Usage: make <command>"
	@echo "Available commands:"
	@awk \
		'BEGIN {FS = ":.*?## "} \
		/^[a-zA-Z_-]+:.*?## / \
		{printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}' \
		$(MAKEFILE_LIST)
