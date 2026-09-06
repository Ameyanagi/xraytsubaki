"""Restore missing declared license files from the exact source commit of an sdist."""
import argparse
from email.parser import BytesParser
import hashlib
import io
from pathlib import Path
import subprocess
import tarfile


def repair(archive, manifest, output, commit):
    with archive.open("rb") as stream:
        original_digest = hashlib.file_digest(stream, "sha256").hexdigest()
    expected = dict(line.split("  ", 1)[::-1] for line in manifest.read_text().splitlines())
    if expected.get(archive.name) != original_digest:
        raise ValueError("Source archive differs from its qualified build checksum")
    stem = archive.name.removesuffix(".tar.gz")
    with tarfile.open(archive) as source:
        metadata = BytesParser().parsebytes(source.extractfile(f"{stem}/PKG-INFO").read())
        if stem != f"{metadata['Name']}-{metadata['Version']}":
            raise ValueError("Archive name does not match package metadata")
        licenses = metadata.get_all("License-File", [])
        if not licenses or set(licenses) - {"LICENSE", "LICENSE-MIT", "LICENSE-APACHE"}:
            raise ValueError("Unexpected license declarations; manual inspection required")
        names = set(source.getnames())
        missing = [name for name in licenses if f"{stem}/{name}" not in names]
        if not missing:
            raise ValueError("No missing license files to repair")
        additions = {name: subprocess.check_output(["git", "show", f"{commit}:{name}"])
                     for name in missing}
        output.mkdir(parents=True, exist_ok=True)
        final = output / archive.name
        if final.resolve() == archive.resolve() or final.exists():
            raise ValueError("Repair output must be a new archive")
        with tarfile.open(final, "w:gz") as dest:
            for member in source.getmembers():
                dest.addfile(member, source.extractfile(member) if member.isfile() else None)
            for name, contents in additions.items():
                member = tarfile.TarInfo(f"{stem}/{name}")
                member.size = len(contents)
                member.mode = 0o644
                dest.addfile(member, io.BytesIO(contents))
    with tarfile.open(archive) as original, tarfile.open(final) as repaired:
        for member in original.getmembers():
            if member.isfile() and original.extractfile(member).read() != repaired.extractfile(member.name).read():
                raise ValueError("Repair changed an existing source file")
        for name in licenses:
            if not repaired.getmember(f"{stem}/{name}").isfile():
                raise ValueError("A declared license is still missing")
    with final.open("rb") as stream:
        checksum = hashlib.file_digest(stream, "sha256").hexdigest()
    Path(str(final) + ".sha256").write_text(f"{checksum}  {final.name}\n")
    print(f"Restored {', '.join(missing)} from {commit}; existing source bytes preserved")
    return final


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["archive", "manifest", "output"]:
        parser.add_argument(name, type=Path)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    repair(args.archive, args.manifest, args.output, args.commit)
