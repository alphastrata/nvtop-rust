import socket
import json
import sys

SOCK_PATH = "/tmp/hayaku.sock"
TARGET_LINES = 1000
OUTPUT_FILE = "telemetry_1k.jsonl"

def main():
    print(f"Connecting to {SOCK_PATH}...")
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        sock.connect(SOCK_PATH)
    except FileNotFoundError:
        print("Socket not found. Is the daemon running?")
        sys.exit(1)
    
    print("Connected. Fetching data...")

    lines_received = 0
    with open(OUTPUT_FILE, "w") as f:
        buffer = b""
        sock.settimeout(5.0) # 5s timeout per recv to handle stalls gracefully
        try:
            while lines_received < TARGET_LINES:
                chunk = sock.recv(4096)
                if not chunk:
                    print("Server closed connection early.")
                    break
                buffer += chunk
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    if not line.strip():
                        continue
                    try:
                        data = json.loads(line.decode("utf-8"))
                        f.write(json.dumps(data) + "\n")
                        lines_received += 1
                        if lines_received % 250 == 0:
                            print(f"Accrued {lines_received}/{TARGET_LINES} records...")
                    except json.JSONDecodeError as e:
                        print(f"Skipping malformed JSON at line {lines_received+1}: {e}")
        except socket.timeout:
            print("Timeout waiting for data stream.")
        finally:
            sock.close()

    print(f"Done! Wrote exactly {lines_received} entries to {OUTPUT_FILE}")

if __name__ == "__main__":
    main()
