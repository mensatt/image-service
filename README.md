# Mensatt Image Processing Service

## Executing in prod command commands

```
docker compose up -d
```

If you want to rebuild the image manually you can use

```
docker compose build
```

## Development commands

```bash
cargo build
cargo install cargo-watch
RUST_LOG=mensatt_img=debug cargo watch -w src -x run
```

## Environment Variables

| Name                 | Description                            | Default |
| -------------------- | -------------------------------------- | ------- |
| `MAX_UPLOAD_SIZE_MB` | Maximum size of an image in Mega-Bytes | `10`    |
