VERSION = 0.4.1

.PHONY: all build clean lint fmt test check publish

all: build

build:
	@cargo build --release -p lidoff

clean:
	@cargo clean

lint:
	@cargo clippy --all

fmt:
	@cargo fmt --all
	@find \
		crates/ \
		-iname '*.h' -o -iname '*.c' \
		| xargs clang-format -i

test:
	@cargo test --workspace

check: test
	@cargo fmt --all --check
	@cargo clippy --all

publish:
	@sed -E 's/^version = "[^"]+"/version = "${VERSION}"/' Cargo.toml > Cargo.toml.tmp
	@mv Cargo.toml.tmp Cargo.toml
	@cargo update -p lidoff
	@git add Makefile Cargo.toml Cargo.lock
	@git commit -m "chore: release ${VERSION} 🔥"
	@git tag "v${VERSION}"
	@git-cliff -o CHANGELOG.md
	@git tag -d "v${VERSION}"
	@git add CHANGELOG.md
	@git commit --amend --no-edit
	@git tag -a "v${VERSION}" -m "release v${VERSION}"
	@git push
	@git push --tags
