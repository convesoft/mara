# Mara

Mara is a Git-native, schema-driven engineering knowledge system for structured
Markdown requirements, traceability, and deterministic context for humans and
engineering agents.

The project is bootstrapping: Mara's own engineering contracts already use the
Mara language, while the Rust implementation is being built to validate that
corpus.

## Project contracts

- [Documentation index](docs/index.mara.md)
- [Product charter](docs/product/charter.mara.md)
- [Human and agent workflows](docs/product/workflows.mara.md)
- [Self-hosting profile](docs/product/self-hosting-profile.mara.md)
- [Verification strategy](docs/verification/strategy.mara.md)

## Development

The initial Rust crate builds with the stable Rust toolchain. Before opening a
pull request, run:

```shell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the authority boundaries between Mara,
Linear, and GitHub and for the contribution workflow.

## Bootstrap acceptance

From the repository root, build the CLI and validate Mara's canonical corpus
with the exact bootstrap acceptance command:

```shell
cargo build --locked --bin mara
./target/debug/mara check --format json
```

CI runs the built `mara check` command twice and compares the complete JSON
outputs byte for byte.

## License

Mara is licensed under either of the following licenses, at your option:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

See [LICENSE](LICENSE) for the dual-license notice.
