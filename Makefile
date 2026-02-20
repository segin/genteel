.PHONY: all audit benchmark test build clean

all: build test audit

audit:
	python3 scripts/audit_tool.py

benchmark:
	python3 scripts/benchmark_audit_regex.py

test:
	cargo test
	python3 -m unittest discover tests

build:
	cargo build

clean:
	cargo clean
	rm -rf audit_reports benchmark_data.txt
