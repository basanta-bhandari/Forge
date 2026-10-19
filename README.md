# Forge

A small terminal CAD tool for preparing 3D-printing models.

Forge only works in [Kitty](https://sw.kovidgoyal.net/kitty/). It exits with an install message in other terminals.

## Run

Requires Rust 1.88 or newer.

```bash
cargo run --release -- --demo
```

To open a model:

```bash
cargo run --release -- model.stl
```

## SSH

Forge works over SSH when you connect from Kitty. Kitty's SSH kitten forwards the terminal setup needed for graphics.

```bash
kitten ssh user@server
./forgetui --demo
```

## Share it

The Rust equivalent of PyPI is [crates.io](https://crates.io/). After publishing, people can try Forge from Kitty with:

```bash
cargo install forgetui
forgetui --demo
```

For downloadable binaries, attach builds to a GitHub Release and share the release URL.


./publish.sh
```

Use `./publish.sh --dry-run` to build the download without pushing anything.
