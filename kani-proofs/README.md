# Kani Model Checking Proofs

Standalone crate for verifying sorex encoding primitives using Kani.

## Known Issue: Workspace Interference

When running from within the sorex workspace, Kani encounters a "duplicate lang item" error due to conflicting std library versions. This is a Kani/Cargo interaction issue, not a macOS limitation.

**Workaround:** Copy the crate outside the workspace to run proofs:

```bash
cp -r kani-proofs /tmp/kani-proofs
cd /tmp/kani-proofs
cargo kani
```

Or run from CI where this crate can be isolated.

## Running Proofs

```bash
# From outside sorex workspace
cargo kani

# Run specific harness
cargo kani --harness verify_varint_roundtrip

# With verbose output
cargo kani --verbose
```

## Verified Properties

1. **`verify_encode_varint_no_panic`**: Encoding never panics for any u64
2. **`verify_decode_varint_no_panic`**: Decoding never panics for any byte sequence
3. **`verify_varint_roundtrip`**: `decode(encode(x)) == x` for all x
4. **`verify_decode_empty_input`**: Empty input returns EmptyBuffer error
5. **`verify_decode_rejects_overlong`**: Varints > 10 bytes rejected

## CI Integration

```yaml
# .github/workflows/kani.yml
kani:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: model-checking/kani-verifier-action@v1
    - run: cargo kani --manifest-path kani-proofs/Cargo.toml
```

Note: Kani officially supports Linux (x86_64, aarch64), macOS Intel, and macOS ARM64.
