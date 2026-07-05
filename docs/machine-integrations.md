# HOW-TO: Testing Process Tracking & Telemetry Extensions

This guide outlines how to verify the bugfixes and new telemetry daemon features implemented in `nvtop`.

---

## 1. Run the Test Suite

Verify that all unit and integration tests (including the new native process retrieval and `HayakuSender` checks) pass:

```bash
# Test the local hayaku dependency with all features
cargo test --manifest-path ../hayaku/hayaku/Cargo.toml --all-features

# Test the nvtop codebase
cargo test --all-features
```

---

## 2. Testing Container-Safe Native Process Tracking (TUI)

Launch the standard terminal interface and verify process detection:

```bash
cargo run
```

* **Verification**: Scroll down to the **GPU Processes** table. Verify that running GPU graphics/compute processes are listed under correct PIDs and memory usage, showing native process types (`C` or `G`) instead of relying on brittle `/proc` name lookups.

---

## 3. Testing Dynamic PID Hooking (TUI)

To verify target isolation in TUI mode, run `nvtop` hooked onto a specific active process PID (e.g., your terminal shell or an active browser/GPU client PID):

```bash
# Locate an active PID using nvtop or system tools, then run:
cargo run -- --hook-pid <PID>
```

* **Verification**: In the **GPU Processes** section, only the process with the matching `<PID>` should be visible. All other OS process noise is successfully isolated and ignored.

---

## 4. Testing Headless Daemon Mode & Hayaku Telemetry Stream

Run `nvtop` in headless daemon mode to stream line-delimited JSON packets downstream:

```bash
cargo run -- --daemon --output target/ptx/telemetry.stream --delay 500
```

While the daemon is running, inspect the telemetry output file in another terminal:

```bash
tail -f target/ptx/telemetry.stream
```

* **Verification**: Verify that compact JSON lines are appended to the stream at 500ms intervals, matching this structure:

```json
{
  "timestamp_ms": 1783269314000,
  "telemetry_tick": true,
  "gpu_utilization_pct": 0,
  "memory_utilization_pct": 0,
  "sm_clock_mhz": 1410,
  "graphics_clock_mhz": 1410,
  "memory_clock_mhz": 875,
  "temperature_c": 50,
  "power_usage_w": 25.4,
  "vram_total_bytes": 17179869184,
  "vram_used_bytes": 4294967296,
  "hook_pid_active": false,
  "target_pid_allocated_bytes": 0
}
```

---

## 5. Testing Isolated PID Sweeps in Daemon Mode

To verify targeted context extraction via the daemon, launch the daemon while hooking onto a specific target process PID:

```bash
cargo run -- --daemon --hook-pid <PID> --output target/ptx/telemetry_hooked.stream
```

Inspect the output stream:

```bash
tail -f target/ptx/telemetry_hooked.stream
```

* **Verification**: If the target PID is actively using the GPU:
  * `hook_pid_active` will be `true`.
  * `target_pid_allocated_bytes` will show the non-zero VRAM bytes allocated to that process.
