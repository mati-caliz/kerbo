"""Compara lo que dejó kerbo en target/fix-dump con los `.fixed`/`.unsafe` de ruff."""
import difflib
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from ruff_fixed import FIXTURES, REPO, golden_stem, registered_fixtures

DUMP = REPO / "target" / "fix-dump"


def main():
    limit = int(sys.argv[1]) if len(sys.argv) > 1 else 30
    differing = 0
    for logical, file in registered_fixtures():
        flat = logical.replace("/__init__", "_init").replace("/", "_")
        source = (FIXTURES / file).read_text(encoding="utf-8")
        for suffix in ("fixed", "unsafe"):
            dumped = DUMP / f"{flat}.{suffix}"
            if not dumped.exists():
                continue
            golden = FIXTURES / f"{golden_stem(logical)}.{suffix}"
            expected = golden.read_text(encoding="utf-8") if golden.exists() else source
            actual = dumped.read_text(encoding="utf-8")
            if actual == expected:
                continue
            differing += 1
            lines = list(difflib.unified_diff(expected.splitlines(), actual.splitlines(), "ruff", "kerbo", lineterm="", n=1))
            print(f"=== {file} ({suffix})")
            print("\n".join(lines[2 : 2 + limit]))
    print(f"fixtures distintos: {differing}")


if __name__ == "__main__":
    main()
