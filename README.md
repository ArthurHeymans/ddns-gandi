# Rust Gandi DDNS Client

A robust and extensible Dynamic DNS (DDNS) client written in Rust, designed to automatically update DNS records with your current public IP addresses.

## Features

- Supports both IPv4 and IPv6 address updates
- Integrates with Gandi LiveDNS API
- Configurable record management
- Comprehensive error handling and logging
- Extensible architecture for adding new DNS providers
- Fault-tolerant operation with retry mechanisms

## Prerequisites

- Rust toolchain (1.70.0 or later)
- Gandi API key
- Domain managed by Gandi LiveDNS

## Installation

1. Clone the repository:
```bash
git clone https://github.com/yourusername/rust-ddns-client.git
cd rust-ddns-client
```

2. Build the project:
```bash
cargo build --release
```

## Configuration

Create a `.gandi.toml` configuration file in the project root:

```toml
[GANDI]
key = "your_gandi_api_key"

[DNS]
domain = "yourdomain.com"
records = "record"
```

## Usage

Run the DDNS client:

```bash
cargo run --release
```

The client will:
1. Fetch your current public IPv4 and IPv6 addresses
2. Compare them with existing DNS records
3. Update the records if changes are detected

## Error Handling

The client includes comprehensive error handling for:
- Network connectivity issues
- API authentication failures
- DNS record update problems
- Configuration errors

## License

This project is licensed under the MIT License - see the LICENSE file for details.
