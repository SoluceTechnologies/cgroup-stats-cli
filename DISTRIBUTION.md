# Installing cgroup-stats-cli

## apt (Debian / Ubuntu)

    curl -fsSL https://solucetechnologies.github.io/cgroup-stats-cli/cgroup-stats-cli-archive-keyring.gpg \
      | sudo tee /usr/share/keyrings/cgroup-stats-cli.gpg >/dev/null
    echo "deb [signed-by=/usr/share/keyrings/cgroup-stats-cli.gpg] https://solucetechnologies.github.io/cgroup-stats-cli stable main" \
      | sudo tee /etc/apt/sources.list.d/cgroup-stats-cli.list
    sudo apt update && sudo apt install cgroup-stats-cli

## Install from Crates.io

```bash
cargo install cgroup-stats-cli
```

## Manual (.deb / tarball)

Download the `.deb` (amd64/arm64) or `.tar.gz` for your platform from
the [latest release](https://github.com/SoluceTechnologies/cgroup-stats-cli/releases/latest).

## Build from Source

1. Clone the repository:
   ```bash
   git clone https://github.com/SoluceTechnologies/cgroup-stats-cli.git
   cd cgroup-stats-cli
   ```

2. Build and install:
   ```bash
   cargo build --release
   cargo install --path .
   ```


