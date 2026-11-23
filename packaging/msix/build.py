import glob
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
RELEASE = ROOT / "target" / "release"
STAGING = ROOT / "target" / "msix-staging"
EXES = ["tokenwhy-app.exe", "tokenwhy.exe"]


def version():
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    section = re.search(r"\[workspace\.package\](.*?)(\n\[|\Z)", text, re.S)
    return re.search(r'^version\s*=\s*"([^"]+)"', section.group(1), re.M).group(1)


def makeappx():
    fixed = Path(r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\makeappx.exe")
    if fixed.exists():
        return str(fixed)
    found = sorted(glob.glob(r"C:\Program Files (x86)\Windows Kits\10\bin\*\x64\makeappx.exe"))
    if not found:
        sys.exit("makeappx.exe not found (install the Windows SDK)")
    return found[-1]


def main():
    ver = version()
    if not (HERE / "Assets" / "StoreLogo.png").exists():
        subprocess.run([sys.executable, str(HERE / "assets.py")], check=True)
    if STAGING.exists():
        shutil.rmtree(STAGING)
    STAGING.mkdir(parents=True)
    for exe in EXES:
        shutil.copy2(RELEASE / exe, STAGING / exe)
    shutil.copytree(HERE / "Assets", STAGING / "Assets")
    manifest = (HERE / "AppxManifest.xml").read_text(encoding="utf-8")
    manifest = re.sub(r'(<Identity[^>]*Version=")[^"]+', rf"\g<1>{ver}.0", manifest)
    (STAGING / "AppxManifest.xml").write_text(manifest, encoding="utf-8")
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    out = dist / f"tokenwhy_{ver}_x64.msix"
    subprocess.run([makeappx(), "pack", "/d", str(STAGING), "/p", str(out), "/o"], check=True)
    print(out)


if __name__ == "__main__":
    main()
