FROM rust:1.71 as builder
WORKDIR /app
RUN apt update && apt install lld clang -y
COPY . .
RUN cargo build --release

FROM rust:1.71 as runtime
WORKDIR /app
COPY --from=builder /app/target/release/server server
ENTRYPOINT ["./server"]

