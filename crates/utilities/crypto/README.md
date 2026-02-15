# `magnus-crypto`

<a href="https://github.com/refcell/magnus/actions/workflows/ci.yml"><img src="https://github.com/refcell/magnus/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/refcell/magnus/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Cryptographic utilities for Magnus.

## Features

- `test-utils` - Enables test utilities including `threshold_schemes` for generating deterministic threshold BLS signing schemes.

## Usage

```toml
[dependencies]
magnus-crypto = { path = "crates/utilities/crypto" }

# For testing
[dev-dependencies]
magnus-crypto = { path = "crates/utilities/crypto", features = ["test-utils"] }
```

## License

[MIT License](https://github.com/refcell/magnus/blob/main/LICENSE)
