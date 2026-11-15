"""Reject signed APKs whose Android or embedded plugin version is stale."""
import json
from pathlib import Path
import re
import subprocess
import sys
import zipfile


def verify(apk, aapt, manifests):
    versions = {json.loads(Path(path).read_text())["version"] for path in manifests}
    if len(versions) != 1:
        raise ValueError(f"Platform manifest versions disagree: {sorted(versions)}")
    version = versions.pop()
    major, minor, patch = map(int, version.split("."))
    expected_code = major * 1_000_000 + minor * 1_000 + patch
    badging = subprocess.check_output([aapt, "dump", "badging", apk], text=True)
    package = next(line for line in badging.splitlines() if line.startswith("package:"))
    name = re.search(r"versionName='([^']*)'", package)
    code = re.search(r"versionCode='([^']*)'", package)
    if not name or not code or name[1] != version or int(code[1]) != expected_code:
        raise ValueError(f"APK version mismatch: expected {version} ({expected_code}), got {package}")
    with zipfile.ZipFile(apk) as archive:
        embedded = json.loads(archive.read("assets/player-plugin.json"))["manifest"]
    if embedded["version"] != version:
        raise ValueError(f"Embedded plugin version {embedded['version']} != {version}")
    print(f"Verified APK and embedded plugin version {version} ({expected_code})")


if __name__ == "__main__":
    verify(sys.argv[1], sys.argv[2], sys.argv[3:])
