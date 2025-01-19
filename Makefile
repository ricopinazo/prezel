db:
	rm -fr .sqlx || true
	cargo sqlx database reset --database-url=sqlite:src.db -y
	cargo sqlx prepare --database-url=sqlite:src.db
	rm src.db

openapi:
	cp Cargo.toml /tmp/prezel-cargo.backup
	echo '[[bin]]' >> Cargo.toml
	echo 'name = "openapi"' >> Cargo.toml
	echo 'path = "src/openapi.rs"' >> Cargo.toml
	# the export PATH bit is just for vercel CLI to find cargo
	# also OPENSSL_NO_VENDOR=1 is just for prezel to compile in vercel CI
	cargo run --bin openapi || (make restore && false)
	make restore

restore:
	mv /tmp/prezel-cargo.backup Cargo.toml
