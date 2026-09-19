#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///

import argparse
import gzip
import hashlib
import shutil
import tarfile
import tempfile
from pathlib import Path


def require_file(path: Path) -> None:
    if not path.is_file():
        raise SystemExit(f"required package input is missing: {path}")


def write_archive(output: Path, root: Path, epoch: int) -> None:
    with (
        output.open("wb") as raw,
        gzip.GzipFile(
            filename="", mode="wb", fileobj=raw, mtime=epoch, compresslevel=9
        ) as gz,
        tarfile.open(fileobj=gz, mode="w", format=tarfile.USTAR_FORMAT) as archive,
    ):
        for path in sorted(root.iterdir(), key=lambda entry: entry.name):
            info = archive.gettarinfo(str(path), arcname=path.name)
            info.uid = 0
            info.gid = 0
            info.uname = ""
            info.gname = ""
            info.mtime = epoch
            info.mode = 0o755 if path.name == "looprs" else 0o644
            with path.open("rb") as source:
                archive.addfile(info, source)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Build a reproducible Unix looprs release"
    )
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--readme", required=True, type=Path)
    parser.add_argument("--license", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--epoch", required=True, type=int)
    args = parser.parse_args()

    for path in (args.binary, args.readme, args.license):
        require_file(path)
    if args.epoch < 0:
        raise SystemExit("epoch must be non-negative")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        shutil.copy2(args.binary, root / "looprs")
        shutil.copy2(args.readme, root / "README.md")
        shutil.copy2(args.license, root / "LICENSE")
        verification = args.output.with_suffix(args.output.suffix + ".verify")
        write_archive(args.output, root, args.epoch)
        write_archive(verification, root, args.epoch)
        if args.output.read_bytes() != verification.read_bytes():
            raise SystemExit("same-commit archive reproducibility check failed")
        verification.unlink()

    digest = hashlib.sha256(args.output.read_bytes()).hexdigest()
    checksum = args.output.with_suffix(args.output.suffix + ".sha256")
    checksum.write_text(f"{digest}  {args.output.name}\n", encoding="ascii")


if __name__ == "__main__":
    main()
