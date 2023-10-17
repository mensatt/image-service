# Mensatt Image Processing Service

## Development commands

```bash 
cargo build
cargo install cargo-watch
RUST_LOG=mensatt_img=debug cargo watch -w src -x run
```
## Environment Variables

| Name                 | Description                            | Default |
|----------------------|----------------------------------------|---------|
| `MAX_UPLOAD_SIZE_MB` | Maximum size of an image in Mega-Bytes | `10`    |
