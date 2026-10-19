# Build from source

You need a recent stable Rust, CMake and a C++ compiler. On Linux you also need:

```sh
sudo apt install cmake libfontconfig-dev libxkbcommon-dev libasound2-dev libclang-dev
```

Then, from a clone of the
[repository](https://github.com/sebastian-beca/opendj):

```sh
cargo run --release
```

## Building these docs

The docs are an [mdBook](https://rust-lang.github.io/mdBook/) in
`website/book`. To preview them:

```sh
cd website/book
mdbook serve --open
```
