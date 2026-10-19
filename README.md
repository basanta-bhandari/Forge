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

## Share it

The Rust equivalent of PyPI is [crates.io](https://crates.io/). After publishing, people can try Forge from Kitty with:

```bash
cargo install forgetui
forgetui --demo
```

For downloadable binaries, attach builds to a GitHub Release and share the release URL.

## Publish

Install and authenticate the [GitHub CLI](https://cli.github.com/), then run:

```bash
./publish.sh
```

Use `./publish.sh --dry-run` to build the download without pushing anything.
