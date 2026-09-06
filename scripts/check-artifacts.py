"""Create/verify checksums with flat asset names suitable for GitHub Releases."""
import argparse
import hashlib
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("mode", choices=["create", "verify"])
parser.add_argument("artifacts", type=Path)
parser.add_argument("manifest", type=Path)
args = parser.parse_args()
assets = {}
for path in sorted(args.artifacts.rglob("*")):
    if not path.is_file() or path.resolve() == args.manifest.resolve():
        continue
    if path.name in assets:
        raise SystemExit(f"Duplicate release asset name: {path.name}")
    assets[path.name] = hashlib.file_digest(path.open("rb"), "sha256").hexdigest()
if not assets:
    raise SystemExit("No release artifacts found")
if args.mode == "create":
    args.manifest.write_text("".join(f"{assets[name]}  {name}\n" for name in sorted(assets)))
else:
    expected = {}
    for line in args.manifest.read_text().splitlines():
        digest, name = line.split("  ", 1)
        if name in expected:
            raise SystemExit(f"Duplicate checksum entry: {name}")
        expected[name] = digest
    if assets != expected:
        raise SystemExit("Release artifacts do not match the qualified build checksums")
    print(f"Verified {len(assets)} GitHub-built release artifacts")
