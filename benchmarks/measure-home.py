import argparse
import subprocess


parser = argparse.ArgumentParser()
parser.add_argument("--requests", type=int, default=10)
args = parser.parse_args()

if not 1 <= args.requests <= 100:
    parser.error("--requests must be between 1 and 100")

command = [
    "curl.exe",
    "--silent",
    "--show-error",
    "--output", "NUL",
    "--max-time", "2",
    "--write-out",
    "status=%{http_code} "
    "ttfb_s=%{time_starttransfer} "
    "total_s=%{time_total}",
    "http://127.0.0.1:3000/",
]

for number in range(1, args.requests + 1):
    result = subprocess.run(command, capture_output=True, text=True)
    output = result.stdout.strip()

    print(f"request={number} {output}", flush=True)

    if result.returncode != 0:
        raise SystemExit(
            f"Request {number} failed: {result.stderr.strip()}"
        )

    if not output.startswith("status=200 "):
        raise SystemExit(
            f"Request {number} returned an unexpected HTTP status."
        )