# HOW-TO: Testing Process Tracking & Telemetry Extensions

This guide outlines how to verify the bugfixes and new telemetry daemon features implemented in `nvtop`.

______________________________________________________________________

## 1. Run the Test Suite

Verify that all unit and integration tests (including the new native process retrieval and `HayakuSender` checks) pass:

```bash
# Test the local hayaku dependency with all features
cargo test --manifest-path ../hayaku/hayaku/Cargo.toml --all-features

# Test the nvtop codebase
cargo test --all-features
```

______________________________________________________________________

## 2. Testing Container-Safe Native Process Tracking (TUI)

Launch the standard terminal interface and verify process detection:

```bash
cargo run
```

- **Verification**: Scroll down to the **GPU Processes** table. Verify that running GPU graphics/compute processes are listed under correct PIDs and memory usage, showing native process types (`C` or `G`) instead of relying on brittle `/proc` name lookups.

______________________________________________________________________

## 3. Testing Dynamic PID Hooking (TUI)

To verify target isolation in TUI mode, run `nvtop` hooked onto a specific active process PID (e.g., your terminal shell or an active browser/GPU client PID):

```bash
# Locate an active PID using nvtop or system tools, then run:
cargo run -- --hook-pid <PID>
```

- **Verification**: In the **GPU Processes** section, only the process with the matching `<PID>` should be visible. All other OS process noise is successfully isolated and ignored.

______________________________________________________________________

## 4. Testing Headless Daemon Mode & Telemetry Stream

Run `nvtop` in headless daemon mode to stream line-delimited JSON packets downstream:

```bash
cargo run -- --daemon --output target/ptx/telemetry.stream --delay 500
```

While the daemon is running, inspect the telemetry output file in another terminal:

```bash
tail -f target/ptx/telemetry.stream
```

- **Verification**: Verify that compact JSON lines are appended to the stream at 500ms intervals, matching the structure of `TelemetryExportPacket` in `src/daemon_processor.rs`. The daemon emits one packet per GPU; multi-GPU consumers should run one daemon per GPU or disambiguate packets by the emitting instance.

### Stream bounding

By default the file stream is capped at **1 MiB** to prevent indefinite growth. Override with `--stream-cap-mb`:

```bash
cargo run -- --daemon --output target/ptx/telemetry.stream --stream-cap-mb 5
```

- **Verification**: Let the daemon run for a while and check that the file size does not exceed the configured cap.

______________________________________________________________________

## 5. Testing Isolated PID Sweeps in Daemon Mode

To verify targeted context extraction via the daemon, launch the daemon while hooking onto a specific target process PID:

```bash
cargo run -- --daemon --hook-pid <PID> --output target/ptx/telemetry_hooked.stream
```

Inspect the output stream:

```bash
tail -f target/ptx/telemetry_hooked.stream
```

- **Verification**: If the target PID is actively using the GPU:
  - `hook_pid_active` will be `true`.
  - `target_pid_allocated_bytes` will show the non-zero VRAM bytes allocated to that process.
