"""Build identities shared by packaging and macOS signing."""
import re


def app_name(channel):
    if channel not in {"stable", "nightly"}:
        raise ValueError("Unknown desktop channel")
    return "rexafs Nightly.app" if channel == "nightly" else "rexafs.app"


def identity(version, environment):
    channel = environment.get("REXAFS_BUILD_CHANNEL", "stable")
    app_name(channel)
    tag = environment.get("REXAFS_BUILD_TAG", "v" + version)
    if channel == "nightly":
        if not re.fullmatch(r"nightly-\d{8}-\d+", tag) or not environment.get("REXAFS_BUILD_UTC"):
            raise ValueError("Nightly builds require an immutable dated tag and build timestamp")
    elif tag != "v" + version:
        raise ValueError("Stable build tag must match the package version")
    return {"channel": channel, "release_tag": tag,
            "built_at": environment.get("REXAFS_BUILD_UTC")}
