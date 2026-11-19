#!/usr/bin/env python3
"""Put the licence and notice files into a built `.hmod`, deterministically.

`cargo horizon-mod build` packs only `horizon-mod.toml`, `assets/` and `data/`,
so the MIT/Apache/BSD notice the licence requires in a distributed copy would
never travel with the archive. This appends `LICENSE` and `NOTICE.md` at the
archive root from the port's own files; the content fence ignores them (it reads
only `assets/` and `data/`), and they are added with fixed dates and modes so a
rerun gives the same bytes.

    tools/add-notices.py content/<id>.hmod [...]
"""
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NOTICES = [('LICENSE', 'LICENSE'), ('NOTICE.md', 'LICENSE-NOTICE.md')]
DATE = (2026, 1, 1, 0, 0, 0)


def add(path: Path) -> None:
    with zipfile.ZipFile(path) as archive:
        present = set(archive.namelist())
    missing = [(name, source) for name, source in NOTICES
               if name not in present and (ROOT / source).is_file()]
    if not missing:
        return
    with zipfile.ZipFile(path, 'a', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, source in missing:
            info = zipfile.ZipInfo(name, DATE)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, (ROOT / source).read_bytes())
    print(f'{path}: added {", ".join(name for name, _ in missing)}')


if __name__ == '__main__':
    for argument in sys.argv[1:]:
        add(Path(argument))
