FROM rust:1.77.0-bookworm AS builder

WORKDIR /app
COPY . .
RUN cargo run -p prisma-cli generate
RUN cargo build -p pingxx-proxy-server --locked --release


FROM debian:bookworm-slim AS final

RUN apt-get update && apt-get install -y libssl-dev pkg-config
RUN apt-get install -y ca-certificates

RUN apt-get install -y libmariadb-dev-compat libmariadb-dev

WORKDIR /app
COPY --from=builder /app/target/release/pingxx-proxy-server /app
CMD ["./pingxx-proxy-server"]
