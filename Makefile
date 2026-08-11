.PHONY: build release run cli tui benchmark-pypi benchmark-npm report schedule clean

build:
	cargo build --workspace

release:
	cargo build --release --workspace

cli:
	cargo run -p mirror-cli

tui:
	cargo run -p mirror-tui

benchmark-pypi:
	cargo run -p mirror-cli -- run pypi

benchmark-npm:
	cargo run -p mirror-cli -- run npm

report:
	cargo run -p mirror-cli -- report

schedule:
	cargo run -p mirror-cli -- schedule

clean:
	cargo clean