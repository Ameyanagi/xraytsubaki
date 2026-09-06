"""Archive helpers shared by native packaging and its regression checks."""
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile


def zip_bundle(bundle: Path, archive: Path) -> None:
    """Keep the bundle root and bytes, clamping only unsupported ZIP dates.

    Cargo packages can contain epoch-dated license files. ZIP dates only cover
    1980 through 2107; strict_timestamps=False handles both limits without
    changing source files or their timestamps.
    """
    with ZipFile(archive, "w", compression=ZIP_DEFLATED, strict_timestamps=False) as output:
        for source in [bundle, *sorted(bundle.rglob("*"))]:
            output.write(source, source.relative_to(bundle.parent).as_posix())
