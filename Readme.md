# sysinfo-osc-client-vrc

An OSC client that sends system information to an OSC server, most likely to VRChat.

## Installation

### Install from source

```bash
cargo build --release
```

## Usage

To display all info:

```bash
sysinfo-osc-client-vrc
# or
./sysinfo-osc-client-vrc
# depends on your path
```

On Windows, you can also double click on the executable directly.

For all options:

```bash
sysinfo-osc-client-vrc --help
```

Example message:

```text
04/17/2024 00:09:07 UTC-04
CPU: 16.09%, Processes: 339
RAM: 15.8 GiB (49.75%)
GPU: 46% (33.67W, 49°C)
4.9 GiB (61.00%)
```
