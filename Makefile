db:
	cargo sqlx database reset --database-url=sqlite:src.db -y

openapi-hash:
	@sha1sum docs/public/openapi.json

openapi-check:
	test "$(shell make openapi-hash)" = "$(shell make openapi > /dev/null 2> /dev/null && make openapi-hash)"

openapi:
	cp Cargo.toml /tmp/prezel-cargo.backup
	echo '[[bin]]' >> Cargo.toml
	echo 'name = "openapi"' >> Cargo.toml
	echo 'path = "src/openapi.rs"' >> Cargo.toml
	cargo run --bin openapi || (make restore && false)
	make restore

restore:
	mv /tmp/prezel-cargo.backup Cargo.toml
