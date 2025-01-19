db:
	rm -fr .sqlx || true
	cargo sqlx database reset --database-url=sqlite:src.db -y
	cargo sqlx prepare --database-url=sqlite:src.db
	rm src.db
