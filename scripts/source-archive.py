"""Archive the app sources and lockfiles used for this build."""
from pathlib import Path
from sys import argv
from zipfile import ZipFile, ZIP_DEFLATED

root = Path(__file__).resolve().parents[1]
include = [
    "package.json", "package-lock.json", "vite.config.js", "index.html", "LICENSE",
    "README.md", "packaging", "src", "src-tauri", "scripts", "vendor",
    "tests", "docs/界面素材/AU05.png", "docs/独立设备接入.md",
    "docs/UI设计规范.md", "docs/AU05开发设计.md", "docs/功能与页面规格.md", "docs/构建发布与验收.md",
]
with ZipFile(argv[1], "w", compression=ZIP_DEFLATED) as archive:
    for name in include:
        base = root / name
        if base.is_file():
            archive.write(base, name)
            continue
        for folder, subfolders, names in base.walk():
            subfolders[:] = sorted(d for d in subfolders if d not in {"target", "gen", "__pycache__"})
            for filename in sorted(names):
                path = folder / filename
                archive.write(path, path.relative_to(root))
