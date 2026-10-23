"""Exercise a real Linux desktop window under X11 (including Xvfb).

Requires xdotool, ImageMagick's import command, and Pillow. Screenshots are
retained for review; this checks launch, rendering, keyboard input and resize,
not physical GPU performance or native Wayland behavior.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time

from PIL import Image, ImageChops, ImageStat


def command(*args, check=True):
    return subprocess.run(args, text=True, capture_output=True, check=check)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("project", type=Path, help="An embedded project fixture")
    parser.add_argument("--output", type=Path, default=Path("target/gui-smoke"))
    args = parser.parse_args()
    if not os.environ.get("DISPLAY"):
        parser.error("Run inside an X11 session or xvfb-run")
    for executable in ["xdotool", "import"]:
        if not shutil.which(executable):
            parser.error(f"Missing {executable}")
    binary = args.binary.resolve(strict=True)
    project = args.project.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    checks = []
    with tempfile.TemporaryDirectory(prefix="rexafs-gui-") as temporary:
        # Exercise a path containing spaces and non-ASCII characters, and keep
        # the retained compatibility fixture untouched.
        copied = Path(temporary) / "copper 日本語.rxs"
        shutil.copy2(project, copied)
        with (output / "desktop.log").open("w") as log:
            process = subprocess.Popen(
                [str(binary), str(copied)], cwd=temporary,
                env={**os.environ, "REXAFS_DEBUG_STATS": "1",
                     "REXAFS_SETTINGS": str(Path(temporary) / "settings.json")},
                stdout=log, stderr=subprocess.STDOUT,
            )
            try:
                deadline = time.monotonic() + 60
                window = None
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise RuntimeError(f"Desktop exited at startup: {process.returncode}")
                    result = command("xdotool", "search", "--onlyvisible", "--pid",
                                     str(process.pid), "--name", "rexafs", check=False)
                    if result.returncode == 0 and result.stdout.strip():
                        window = result.stdout.splitlines()[0]
                        break
                    time.sleep(0.2)
                if window is None:
                    raise RuntimeError("Desktop did not open a visible window")
                command("xdotool", "windowfocus", "--sync", window)
                geometry = dict(line.split("=", 1) for line in command(
                    "xdotool", "getwindowgeometry", "--shell", window).stdout.splitlines())
                desktop = command("xdotool", "getdisplaygeometry").stdout.split()
                assert int(geometry["WIDTH"]) <= int(desktop[0]), geometry
                assert int(geometry["HEIGHT"]) <= int(desktop[1]), geometry
                checks.append("window-fits-display")
                # Select Process, then its raw μ(E) tab. The diagnostics report
                # this first plot's renderer. The quantity tabs precede the
                # comparison controls, just after the default 280 px Groups
                # panel. Coordinates are window-relative at the X11 smoke
                # session's unscaled laptop resolution.
                command("xdotool", "key", "--clearmodifiers", "--delay", "100", "ctrl+2")
                time.sleep(0.5)
                command("xdotool", "mousemove", "--window", window, "310", "132", "click", "1")
                # Wait for restored data and plot construction, not just the
                # first empty frame. Diagnostics explicitly distinguish these.
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise RuntimeError("Desktop exited while restoring the project")
                    if re.search(r"ruviz frames [1-9]\d*/s", (output / "desktop.log").read_text()):
                        break
                    time.sleep(0.2)
                else:
                    raise RuntimeError("Project did not produce a stage plot")

                def capture(name):
                    if process.poll() is not None:
                        raise RuntimeError(f"Desktop exited during {name}")
                    path = output / f"{name}.png"
                    command("import", "-window", window, str(path))
                    with Image.open(path) as image:
                        frame = image.convert("RGB")
                    if max(ImageStat.Stat(frame).stddev) < 8:
                        raise RuntimeError(f"Blank desktop frame: {name}")
                    return frame

                previous = capture("opened-project")
                checks.append("embedded-project-and-rendered-plot")
                command("xdotool", "mousemove", "--window", window, "30", "20")
                for key, name in [("ctrl+1", "data"), ("ctrl+2", "process"),
                                  ("ctrl+3", "fit"), ("ctrl+5", "publish")]:
                    command("xdotool", "windowfocus", "--sync", window,
                            "key", "--clearmodifiers", "--delay", "100", key)
                    time.sleep(1)
                    frame = capture(name)
                    # The first key may select the restored stage again.
                    if name != "data" and ImageChops.difference(previous, frame).getbbox() is None:
                        raise RuntimeError(f"Shortcut did not change the frame: {key}")
                    previous = frame
                    checks.append(f"shortcut-{name}")
                command("xdotool", "windowsize", "--sync", window, "960", "640",
                        "key", "--clearmodifiers", "ctrl+2")
                time.sleep(1)
                frame = capture("small-window")
                assert frame.size == (960, 640), frame.size
                checks.append("resize-960x640")
                command("xdotool", "windowfocus", "--sync", window,
                        "key", "--clearmodifiers", "ctrl+q")
                assert process.wait(timeout=15) == 0
                checks.append("quit-shortcut")
            except Exception:
                # Preserve the visible state even when plot readiness fails,
                # before the normal screenshot sequence has started.
                if window is not None and process.poll() is None:
                    command("import", "-window", window,
                            str(output / "failure.png"), check=False)
                raise
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
    (output / "checks.json").write_text(json.dumps({"checks": checks}, indent=2) + "\n")
    print(f"Linux GUI: {len(checks)} checks passed; screenshots in {output}")


if __name__ == "__main__":
    main()
