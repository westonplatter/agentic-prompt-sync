# Contributing

## Build

```bash
cargo build           # Debug build
cargo build --release # Release build
```

## Run tests

```bash
cargo test
```

## Linting

This project uses [Trunk](https://docs.trunk.io) for linting and code quality checks.

```bash
trunk check       # Run linters on modified files
trunk fmt         # Format code
trunk check list  # View available linters
```

## Run with verbose logging

```bash
cargo run -- --verbose sync
```
