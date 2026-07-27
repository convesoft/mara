# Mara qualification xtask

`mara-xtask` has exactly two operational commands. Run them from the canonical
source repository root with `CARGO_TARGET_DIR` unset and a job-unique absolute
external root:

```sh
cargo build --locked -p mara-xtask
cargo build --locked --release --bin mara

target/debug/mara-xtask qualification generate-scale-v01 \
  --qualification-root "$qualification_root"
target/debug/mara-xtask qualification measure-scale-v01 \
  --qualification-root "$qualification_root"
```

Generation creates only the standalone fixture and evidence directories below
the external root. Measurement is Linux-only, invokes the independent verifier
once, and retains five evidence records. The fixed verifier is invoked from the
source root:

```sh
tests/qualification/verify-scale-v01.sh \
  --qualification-root "$qualification_root"
```

The CI fixture-verification workflow independently recomputes the active
manifest on native Ubuntu and macOS. It deliberately does not run the final
performance qualification or make a platform-support claim; those results are
owned by CON-32.
