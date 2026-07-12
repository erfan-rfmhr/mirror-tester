.PHONY: build release run benchmark-pypi benchmark-npm report schedule clean

build:
	cargo build

release:
	cargo build --release

run:
	cargo run -- tui

benchmark-pypi:
	cargo run -- run pypi

benchmark-npm:
	cargo run -- run npm

report:
	cargo run -- report

schedule:
	cargo run -- schedule

clean:
	cargo clean
