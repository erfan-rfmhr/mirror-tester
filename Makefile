.PHONY: build release run cli tui benchmark-pypi benchmark-npm benchmark-gradle report schedule clean

build:
	cargo build --workspace

release:
	cargo build --release --workspace

cli:
	cargo run -p ayeneh-cli

tui:
	cargo run -p ayeneh-tui

benchmark-pypi:
	cargo run -p ayeneh-cli -- run pypi

benchmark-npm:
	cargo run -p ayeneh-cli -- run npm

benchmark-gradle:
	cargo run -p ayeneh-cli -- run gradle

report:
	cargo run -p ayeneh-cli -- report

schedule:
	cargo run -p ayeneh-cli -- schedule

clean:
	cargo clean