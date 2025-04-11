FROM rust:slim

RUN cargo install sqlx-cli --no-default-features --features postgres

WORKDIR /app

ENTRYPOINT ["sqlx", "migrate", "run"]
