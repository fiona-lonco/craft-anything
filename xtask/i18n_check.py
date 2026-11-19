"""Check a UI translation catalog against the Spanish one (whose first column is the full key list).

    python -I xtask/i18n_check.py crates/ui-egui/src/i18n/<code>.tsv

Rules (the same the app's catalog tests use):
- every line is `English key<TAB>translation`; `#` lines are comments; no other tabs;
- exactly the keys of crates/ui-egui/src/i18n/es.tsv, each once;
- a translation keeps its key's {placeholders} (the text between braces), its literal `\\n`
  escapes, and ends with `…` exactly when the key does;
- a translation isn't empty.
It also reports how many translations are identical to the English key (product names and
codes may be; many more means untranslated text).
Exit code 0 when the catalog passes.
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REFERENCE = ROOT / "crates" / "ui-egui" / "src" / "i18n" / "es.tsv"


def placeholders(s: str) -> list:
    """As the Rust test: for each '{', the text up to the next '}', sorted."""
    out = []
    for part in s.split("{")[1:]:
        if "}" in part:
            out.append(part.split("}", 1)[0])
    return sorted(out)


def read(path: Path):
    entries, errors = [], []
    for n, line in enumerate(path.read_bytes().decode("utf-8").splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        if "\t" not in line:
            errors.append(f"{path.name}:{n}: no tab")
            continue
        key, value = line.split("\t", 1)
        if "\t" in value:
            errors.append(f"{path.name}:{n}: more than one tab")
        entries.append((n, key, value))
    return entries, errors


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    path = Path(sys.argv[1])
    ref, _ = read(REFERENCE)
    want = [k for _, k, _ in ref]
    got, errors = read(path)
    seen = {}
    same = 0
    for n, key, value in got:
        if key in seen:
            errors.append(f"{path.name}:{n}: duplicate key (first at line {seen[key]}): {key[:80]}")
            continue
        seen[key] = n
        if not key or not value.strip():
            errors.append(f"{path.name}:{n}: empty key or translation")
            continue
        if placeholders(key) != placeholders(value):
            errors.append(f"{path.name}:{n}: placeholders differ {placeholders(key)} vs {placeholders(value)}: {key[:80]}")
        if key.count("\\n") != value.count("\\n"):
            errors.append(f"{path.name}:{n}: \\n escapes differ ({key.count(chr(92) + 'n')} vs {value.count(chr(92) + 'n')}): {key[:80]}")
        if key.endswith("…") != value.endswith("…"):
            errors.append(f"{path.name}:{n}: must end with … exactly when the key does: {key[:80]}")
        if value.strip() == key.strip():
            same += 1
    missing = [k for k in want if k not in seen]
    extra = [k for k in seen if k not in set(want)]
    for k in missing[:40]:
        errors.append(f"missing key: {k[:100]}")
    if len(missing) > 40:
        errors.append(f"... and {len(missing) - 40} more missing keys")
    for k in extra[:40]:
        errors.append(f"key not in es.tsv (remove it): {k[:100]}")
    for e in errors[:200]:
        print(e)
    print(f"{path.name}: {len(seen)} of {len(want)} keys, {len(missing)} missing, {len(extra)} extra, {same} identical to English, {len(errors)} problems")
    if same > len(want) * 0.15:
        print(f"WARNING: {same} translations are identical to English; translate them unless they are names or codes.")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
