"""Record source/binary hashes after the documented locked release build commands."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[2]


def inside(value):
    path = (ROOT / value).resolve()
    if not path.is_relative_to(ROOT):
        raise ValueError("Build artifacts must remain inside the repository")
    return path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", type=Path, default=Path("benchmarks/diagnostics/stages-config.json"))
    args = parser.parse_args()
    config = json.loads(inside(args.config).read_text(encoding="utf-8-sig"))
    output = inside(config["build_manifest"])
    if output.exists():
        raise ValueError("Retained build metadata must not be overwritten")
    source = config["application_source_revision"]
    paths = ["crates/rubicon/Cargo.toml", "crates/rubicon/Cargo.lock", "crates/rubicon/js/web.js"]
    paths += [path.relative_to(ROOT).as_posix() for path in sorted((ROOT / "crates/rubicon/src").glob("*.rs"))]
    subprocess.run(["git", "diff", "--exit-code", source, "--", *paths], cwd=ROOT, check=True,
                   stdout=subprocess.DEVNULL)
    paths += [config["binary_paths"][variant] for variant in ("off", "on")]
    hashes = [{"path": path, "sha256": hashlib.sha256(inside(path).read_bytes()).hexdigest()} for path in paths]
    base = "cargo build --release --locked --offline --jobs 4 --manifest-path crates/rubicon/Cargo.toml --bin rubicon"
    value = {"application_source_revision": subprocess.check_output(["git", "rev-parse", source], cwd=ROOT, text=True).strip(),
             "profile": "release", "commands": [base, base + " --features render-timing"],
             "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
             "cargo": subprocess.check_output(["cargo", "--version"], text=True).strip(),
             "rustflags": os.environ.get("RUSTFLAGS"), "hashes": hashes}
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
