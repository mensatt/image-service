# Builder
FROM rust:1.73.0-alpine3.18 as builder

RUN apk upgrade --no-cache && apk add --no-cache musl-dev vips-dev
WORKDIR /usr/src/mensatt-img
COPY Cargo.lock Cargo.toml ./
COPY src ./src

# https://stackoverflow.com/a/71669101
RUN RUSTFLAGS="-C target-feature=-crt-static $(pkg-config vips --libs)" cargo install --target x86_64-unknown-linux-musl --path .

# Runner
FROM alpine:3.18.4
RUN apk upgrade --no-cache && apk --no-cache add vips
COPY --from=builder /usr/local/cargo/bin/mensatt-img /usr/local/bin/mensatt-img
EXPOSE 3000
VOLUME /uploads
CMD ["mensatt-img"]