#!/usr/bin/env python3
"""Telemetry Data Fetcher & Cleaner for nvtop Daemon.

Usage:
    # TCP mode (daemon running on 127.0.0.1:9990):
    python scripts/fetch_telemetry.py --tcp 127.0.0.1:9990 --target 1000 -o telemetry_tcp.jsonl

    # File mode (daemon writing to file):
    python scripts/fetch_telemetry.py --input /tmp/telemetry_raw.jsonl --target 1000 -o telemetry_1k.jsonl

Author: Jer & Camuward (nvtop-rust)
"""

import argparse
import json
import socket
import sys
import time


def fetch_tcp(addr: str, target: int, out_path: str) -> None:
    """Connect via TCP and stream JSONL lines."""
    host, port = addr.split(":")
    port = int(port)

    print(f"[FETCHER] Connecting to {addr}")
    sock = socket.create_connection((host, port))
    print(f"[FETCHER] Connected! Collecting {target} records...")

    buf = b""
    collected = 0

    try:
        while collected < target:
            chunk = sock.recv(4096)
            if not chunk:
                print("[FETCHER] Server disconnected unexpectedly.")
                break

            buf += chunk
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                try:
                    data = json.loads(line.decode("utf-8"))
                    with open(out_path, "a") as f_out:
                        f_out.write(json.dumps(data) + "\n")
                    collected += 1
                    if collected % 250 == 0:
                        print(
                            f"[FETCHER] Accrued {collected}/{target} clean records..."
                        )
                except (json.JSONDecodeError, UnicodeDecodeError):
                    pass

    except KeyboardInterrupt:
        pass

    finally:
        sock.close()

    print(f"[FETCHER] Done! Collected exactly {collected} entries into {out_path}")


def fetch_file(input_path: str, target: int, out_path: str) -> None:
    """Tail a file and extract valid JSONL lines."""
    print(f"[FETCHER] Waiting for data in '{input_path}'...")

    collected = 0
    with open(input_path, "r") as f_in:
        try:
            while collected < target:
                line = f_in.readline()
                if not line:
                    time.sleep(0.1)
                    f_in.seek(0, 2)
                    continue

                line = line.strip()
                if not line:
                    continue

                try:
                    data = json.loads(line)
                    with open(out_path, "a") as f_out:
                        f_out.write(json.dumps(data) + "\n")
                    collected += 1
                    if collected % 250 == 0:
                        print(
                            f"[FETCHER] Accrued {collected}/{target} clean records..."
                        )
                except json.JSONDecodeError:
                    pass
        except KeyboardInterrupt:
            pass

    print(f"[FETCHER] Done! Collected exactly {collected} entries into {out_path}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Fetch and clean telemetry data from nvtop daemon."
    )
    file_group = parser.add_mutually_exclusive_group(required=True)
    file_group.add_argument(
        "--input", type=str, help="Raw output file from the daemon (file mode)"
    )
    file_group.add_argument(
        "--tcp", type=str, help="TCP address to connect to (e.g. 127.0.0.1:9990)"
    )
    parser.add_argument(
        "--target", type=int, default=1000, help="Number of valid records to collect"
    )

    parser.add_argument(
        "-o",
        "--output",
        type=str,
        default="telemetry_1k.jsonl",
        help="Clean JSONL output file",
    )

    args = parser.parse_args()

    # Clean start: remove existing output
    try:
        import os

        os.remove(args.output)
    except FileNotFoundError:
        pass

    if args.tcp:
        fetch_tcp(args.tcp, args.target, args.output)
    else:
        assert args.input is not None
        fetch_file(args.input, args.target, args.output)


if __name__ == "__main__":
    main()
