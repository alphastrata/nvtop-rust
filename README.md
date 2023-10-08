# NVTOP

<center>

![img](assets/jensen-banner.png)

An NVIDIA SMI'esk GPU Monitoring tool for your terminal.

_art by stable-diffusion + Maz_

</center>

### Contents:

- [usage](#Usage)
- [prerequisites](#prerequisites)
- [installation](#Installation)
- [why](#why)
- [troubleshooting](#troubleshooting)

`nvtop` is a command-line utility that provides a replacement for some of the output from `nvidia-smi` (System Management Interface).
It offers real-time monitoring and visualization of GPU information: Core Clock, Temps, Fanspeed and Memory Usage.

______________________________________________________________________

# Usage:

```
# Monitor the GPU and system with a 1-second update interval
nvtop --delay 1000
```

______________________________________________________________________

### Prerequisites

Before installing `nvtop`, ensure that you have Rust and Cargo (the Rust package manager) installed on your system. You can download and install Rust from the official website: [Rust Downloads](https://www.rust-lang.org/tools/install).

You will also need to at least confirm that `nvidia-smi` (The official NVIDIA tool that this one seeks to mimic) works.
_Why?_ Because, not all of the functionality from [`nvmlt-sys`](https://crates.io/crates/nvml-sys/versions) the library this app relies on does **not** guarantee all reporting functionality across ALL NVIDIA gpus.

## Installation

### Install via Cargo

You can install `nvtop` directly from Cargo. Follow these steps:

1. Build and install `nvtop` from GitHub:

   ```bash
   cargo install --git https://github.com/alphastrata/nvtop
   ```

### Build manually

To build `nvtop` from the source code, you can follow these steps:

1. Download the source code or clone the repository to your local machine:

   ```bash
   git clone https://github.com/alphastrata/nvtop
   ```

1. Change to the `nvtop` directory:

   ```bash
   cd nvtop
   ```

1. Build the project using Cargo:

   ```bash
   cargo build --release
   ```

1. After building, you can find the `nvtop` executable in the `target/release/` directory.

#### Install build artifact

To make `nvtop` easily accessible from the command line, you can copy the executable to a directory in your system's `PATH`. For example, you can copy it to `/usr/local/bin/`:

```bash
sudo install -Dm755 target/release/nvtop /usr/local/bin/nvtop
```

Now, you can use `nvtop` from anywhere in your terminal.

______________________________________________________________________

# Why?

because \_this:

```shell
+---------------------------------------------------------------------------------------+
| NVIDIA-SMI 535.113.01             Driver Version: 535.113.01   CUDA Version: 12.2     |
|-----------------------------------------+----------------------+----------------------+
| GPU  Name                 Persistence-M | Bus-Id        Disp.A | Volatile Uncorr. ECC |
| Fan  Temp   Perf          Pwr:Usage/Cap |         Memory-Usage | GPU-Util  Compute M. |
|                                         |                      |               MIG M. |
|=========================================+======================+======================|
|   0  NVIDIA TITAN RTX               Off | 00000000:0A:00.0  On |                  N/A |
| 41%   44C    P0              67W / 280W |   1367MiB / 24576MiB |      2%      Default |
|                                         |                      |                  N/A |
+-----------------------------------------+----------------------+----------------------+

+---------------------------------------------------------------------------------------+
| Processes:                                                                            |
|  GPU   GI   CI        PID   Type   Process name                            GPU Memory |
|        ID   ID                                                             Usage      |
|=======================================================================================|
|    0   N/A  N/A      1008      G   /usr/lib/Xorg                               439MiB |
+---------------------------------------------------------------------------------------+
```

is **boring**, and this:
![nvtop](https://raw.githubusercontent.com/alphastrata/nvtop/main/assets/screenshot.png)

is **fun!**




______________________________________________________________________

# Troubleshooting:
If something ain't working please feel free to open an issue, before doing so however, the app has the ability to do some verbose logging (to disk) -- please enable that like so:
```nvtop --logging```
