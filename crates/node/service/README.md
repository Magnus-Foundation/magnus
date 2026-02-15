# `magnus-service`

[![CI](https://github.com/refcell/magnus/actions/workflows/ci.yml/badge.svg)](https://github.com/refcell/magnus/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

Magnus node service orchestration.

## Key Types

- `MagnusNodeService` - Main service type that orchestrates node components

## Usage

```rust,ignore
use magnus_config::NodeConfig;
use magnus_service::MagnusNodeService;

fn main() -> eyre::Result<()> {
    let config = NodeConfig::default();
    let service = MagnusNodeService::new(config);
    service.run()
}
```

## License

[MIT License](https://opensource.org/licenses/MIT)
