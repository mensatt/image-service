# Builder
FROM rust:1.90.0-alpine3.22 AS builder

RUN apk upgrade --no-cache && apk add --no-cache musl-dev vips-dev
WORKDIR /usr/src/mensatt-img
COPY Cargo.lock Cargo.toml ./
COPY src ./src

# https://stackoverflow.com/a/71669101
RUN RUSTFLAGS="-C target-feature=-crt-static $(pkg-config vips --libs)" cargo build --locked --release && \
        cp target/release/mensatt-img /usr/src/mensatt-img/mensatt-img

# Runner
FROM alpine:3.22.2
RUN apk upgrade --no-cache && apk --no-cache add libheif vips 
COPY --from=builder /usr/src/mensatt-img/mensatt-img /usr/local/bin/mensatt-img
EXPOSE 3000
WORKDIR /
CMD ["mensatt-img"]
