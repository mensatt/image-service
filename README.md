# Mensatt Image Service

This is Mensatt's image service. It handles uploads of images and serves them.

## Image Flow

This section defines the steps an image that is uploaded to this service undergoes from upload to serving.

Note: Currently, step 2 is skipped and there is no pruning of uploaded images in step 1, since this service is still under development. Changing both of these things is on the roadmap.

1. **Upload**: Typically images are uploaded to this service _during_ creation of reviews in the frontend. Once uploaded, images are rotated, stripped of their EXIF metadata and saved as AVIF in `PENDING_PATH`.

   Note: Uploading images before a review is submitted is done to speed up the review submission, as the image is likely to be uploaded by the time the user enters their username and/or review text.  
   Also, images that stay in the pending folder for longer than an hour will be deleted regularly.

2. **Submission**: Once a review is submitted, the image is moved from `PENDING_PATH` to `UNAPPROVED_PATH`.
3. **Approval**: Images need to be approved by an administrator. Once an image is approved it is moved fom `UNAPPROVED_PATH` to `ORIGINAL_PATH`.
4. **Serving requests**: The first time an image (with a specific size and quality) is requested, it gets generated from the image in `ORIGINAL_PATH` and cached to `CACHE_PATH`.  
   Every following request (with the same size and quality) gets served from `CACHE_PATH`.

The paths mentioned here are constant and defined in [`src/constants.rs`](https://github.com/mensatt/image-service/blob/main/src/constants.rs).

## API Endpoints

| Name           | Method | Description                                                        | Authorization required? | Already implemented? |
| -------------- | ------ | ------------------------------------------------------------------ | ----------------------- | -------------------- |
| `/upload`      | POST   | Upload an image. <br> Step 1 of [Image Flow](#image-flow).         | no                      | yes                  |
| `/submit/:id`  | POST   | Submit image with `id`. <br> Step 2 of [Image Flow](#image-flow).  | yes                     | yes                  |
| `/approve/:id` | POST   | Approve image with `id`. <br> Step 3 of [Image Flow](#image-flow). | yes                     | yes                  |
| `/image/:id`   | GET    | Get image with `id`. <br> Step 4 of [Image Flow](#image-flow).     | no¹                     | yes                  |
| `/image/:id`   | DELETE | Delete image with `id`. <br> Step 4 of [Image Flow](#image-flow).  | no¹                     | yes²                 |

Authorization is done by providing this header in a request:

```
Authorization: Bearer api_key_goes_here
```

¹: Authorization is required if you want to view unapproved images
²: Images are not currently purged from the cache

## Production usage

1. Clone this repo on the target machine
2. Build and start the service in the background with

   ```
   docker compose up -d
   ```

   The service will be listening on the address defined by `LISTEN_ADDR` in [`src/constants.rs`](https://github.com/mensatt/image-service/blob/main/src/constants.rs)

If needed you can manually (re)build the image with

```
docker compose build
```

## Development usage

1. Make sure to have `cargo-watch` installed by running

   ```
   cargo install cargo-watch
   ```

   Note: `cargo-watch` ist not a requirement for building/running this project, but we found it makes development easier by providing auto reload when a file is changed.
   For more details on what it does see [here](https://crates.io/crates/cargo-watch).

2. Make sure you have `API_KEY_HASH` defined in `.env` (see `.env.dist` for an example) and source it with

   ```
   export $(grep -Ev '^\s*(#|;|/|$)' .env | xargs)
   ```

   Note: The regex ensures empty lines and comments are ignored.  
   If you want to use any other characters for comments in `.env`, make sure to add them to the reqex.

3. You can then build and run the current version of the code with

   ```
   RUST_LOG=mensatt_img=debug cargo watch -w src -x run
   ```

   where `RUST_LOG=mensatt_img=debug` sets the loglevel for `mensatt_img` package to `debug`

   If you do not want to (or cannot) use `cargo-watch`, you can also simply run

   ```
   RUST_LOG=mensatt_img=debug cargo run
   ```

## Environment Variables

| Name           | Description                                                                                                                   | Default | Required? |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------- | ------- | --------- |
| `API_KEY_HASH` | Argon2id hash of the API key to be used. <br> Can be generated [here](https://argon2.online/). Make sure to use Encoded Form. | -       | yes       |

## Contributions
