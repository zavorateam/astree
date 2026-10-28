import fnmatch
import os
import re
from pathlib import Path

BINARY_EXTS = {
    '.o', '.a', '.so', '.dylib', '.dll', '.exe', '.bin', '.lock',
    '.pyc', '.pyd', '.wasm', '.png', '.jpg', '.jpeg', '.gif', '.ico',
    '.pdf', '.zip', '.tar', '.gz', '.bz2', '.xz', '.7z', '.rlib',
    '.pdb', '.d', '.timestamp'
}

DEFAULT_IGNORES = {
    ".git", "target", "build", "dist", "node_modules", 
    ".venv", "venv", "__pycache__", ".pytest_cache"
}

def is_binary(path: Path) -> bool:
    if path.suffix.lower() in BINARY_EXTS:
        return True
    try:
        with open(path, "rb") as f:
            chunk = f.read(1024)
            if b"\x00" in chunk:
                return True
    except Exception:
        return True
    return False

def human_size(bytes_len: int) -> str:
    for unit in ['B', 'K', 'M', 'G', 'T']:
        if bytes_len < 1024:
            if unit == 'B':
                return f"[{bytes_len:>4}{unit}]"
            return f"[{bytes_len:>4.1f}{unit}]".replace('.0', ' ')
        bytes_len /= 1024
    return f"[{bytes_len:>4.1f}P]"

def extract_comment(path: Path) -> str:
    if is_binary(path):
        return ""
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            for _ in range(15):
                line = f.readline()
                if not line:
                    break
                s = line.strip()
                if not s or s.startswith("#!"):
                    continue
                if s.startswith(('"""', "'''")):
                    raw = s.strip("\"'").strip()
                    return re.sub(r'[\x00-\x1f\x7f-\x9f]', '', raw)[:80]
                if path.suffix == ".md" and s.startswith("#"):
                    return s.lstrip("# ").strip()[:80]
                if s.startswith(("//", "#", "<!--", "--")):
                    raw = re.sub(r'^[#/\-<! ]+', '', s).rstrip('-> ').strip()
                    clean = re.sub(r'[\x00-\x1f\x7f-\x9f]', '', raw)
                    return clean[:80]
    except Exception:
        pass
    return ""

def load_gitignore(root: Path) -> list[str]:
    patterns = []
    gi = root / ".gitignore"
    if gi.is_file():
        try:
            with open(gi, "r", encoding="utf-8", errors="ignore") as f:
                for line in f:
                    s = line.strip()
                    if s and not s.startswith("#"):
                        patterns.append(s.rstrip("/"))
        except Exception:
            pass
    return patterns

def dump_tree(root: Path, current: Path, prefix="", level=1, max_level=None,
              show_sizes=False, dirs_only=False, ignore_pat=None, gi_patterns=None):
    if max_level and level > max_level:
        return

    if gi_patterns is None:
        gi_patterns = load_gitignore(root)

    try:
        entries = sorted(list(current.iterdir()), key=lambda e: (not e.is_dir(), e.name.lower()))
    except PermissionError:
        return

    valid = []
    for entry in entries:
        name = entry.name
        rel = str(entry.relative_to(root))
        if name in DEFAULT_IGNORES:
            continue
        if ignore_pat and re.search(ignore_pat, name):
            continue
        if any(fnmatch.fnmatch(name, p) or fnmatch.fnmatch(rel, p) for p in gi_patterns):
            continue
        if dirs_only and not entry.is_dir():
            continue
        valid.append(entry)

    count = len(valid)
    for idx, entry in enumerate(valid):
        is_last = (idx == count - 1)
        connector = "└── " if is_last else "├── "
        next_prefix = prefix + ("    " if is_last else "│   ")

        size_tag = ""
        if show_sizes:
            try:
                size_tag = human_size(entry.stat().st_size) + "  "
            except Exception:
                size_tag = "[   ?]  "

        if entry.is_symlink():
            target = os.readlink(entry)
            print(f"{prefix}{connector}{size_tag}{entry.name} -> {target}")
        elif entry.is_dir():
            print(f"{prefix}{connector}{size_tag}{entry.name}/")
            dump_tree(root, entry, next_prefix, level + 1, max_level,
                      show_sizes, dirs_only, ignore_pat, gi_patterns)
        else:
            is_exec = os.access(entry, os.X_OK)
            star = "*" if is_exec and entry.suffix in {".py", ".sh", ".bash"} else ""
            comment = extract_comment(entry)
            comment_str = f" # {comment}" if comment else ""
            print(f"{prefix}{connector}{size_tag}{entry.name}{star}{comment_str}")
