# lidoff - MacBook lid angle brightness daemon
VERSION = 0.3.2

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
