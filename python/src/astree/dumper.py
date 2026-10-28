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
            val = bytes_len / 1024.0
            if val < 10.0:
                return f"[{val:>4.1f}{unit}]"
            return f"[{int(val):>4}{unit}]"
        bytes_len /= 1024
    return f"[{bytes_len:>4.1f}P]"

def extract_comment(path: Path) -> str:
    if is_binary(path):
        return ""
    ext = path.suffix.lower()
    md_header = None
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            for _ in range(15):
                line = f.readline()
                if not line:
                    break
                s = line.strip()
                if not s or s.startswith("#!"):
                    continue

                if ext == ".rs":
                    if s.startswith(("//!", "///")):
                        clean = s[3:].strip()
                        return re.sub(r'[\x00-\x1f\x7f-\x9f]', '', clean)[:80]
                    if s.startswith("//"):
                        clean = s[2:].strip()
                        return re.sub(r'[\x00-\x1f\x7f-\x9f]', '', clean)[:80]
                    continue

                if ext == ".md":
                    if s.startswith(">"):
                        clean = s.lstrip("> ").strip()
                        return re.sub(r'[\x00-\x1f\x7f-\x9f]', '', clean)[:80]
                    if s.startswith("#") and md_header is None:
                        md_header = s.lstrip("# ").strip()[:80]
                    continue

                if s.startswith(('"""', "'''")):
                    raw = s.strip("\"'").strip()
                    return re.sub(r'[\x00-\x1f\x7f-\x9f]', '', raw)[:80]
                if s.startswith(("//", "#", "<!--", "--")):
                    raw = re.sub(r'^[#/\-<! ]+', '', s).rstrip('-> ').strip()
                    clean = re.sub(r'[\x00-\x1f\x7f-\x9f]', '', raw)
                    return clean[:80]

        if md_header:
            return re.sub(r'[\x00-\x1f\x7f-\x9f]', '', md_header)[:80]
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
                        patterns.append(s.strip("/"))
        except Exception:
            pass
    return patterns

def dump_tree(root: Path, current: Path, prefix="", level=1, max_level=None,
              show_sizes=False, dirs_only=False, ignore_pat=None, gi_patterns=None, out=None):
    if max_level is not None and level > max_level:
        return

    if gi_patterns is None:
        gi_patterns = load_gitignore(root)

    try:
        entries = sorted(list(current.iterdir()), key=lambda e: (not e.is_dir(), e.name.lower(), e.name))
    except PermissionError as e:
        raise PermissionError(f"Cannot read directory '{current}': {e}")

    valid = []
    for entry in entries:
        name = entry.name
        rel = str(entry.relative_to(root))
        if name in DEFAULT_IGNORES:
            continue
        if ignore_pat and re.search(ignore_pat, name):
            continue
        if any(name == p or rel == p or fnmatch.fnmatch(name, p) or fnmatch.fnmatch(rel, p) for p in gi_patterns):
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

        writer = out.write if out else print

        if entry.is_symlink():
            target = os.readlink(entry)
            writer(f"{prefix}{connector}{size_tag}{entry.name} -> {target}\n" if out else f"{prefix}{connector}{size_tag}{entry.name} -> {target}")
        elif entry.is_dir():
            writer(f"{prefix}{connector}{size_tag}{entry.name}/\n" if out else f"{prefix}{connector}{size_tag}{entry.name}/")
            dump_tree(root, entry, next_prefix, level + 1, max_level,
                      show_sizes, dirs_only, ignore_pat, gi_patterns, out=out)
        else:
            is_exec = os.access(entry, os.X_OK) and not entry.is_dir()
            star = "*" if is_exec else ""
            comment = extract_comment(entry)
            comment_str = f" # {comment}" if comment else ""
            writer(f"{prefix}{connector}{size_tag}{entry.name}{star}{comment_str}\n" if out else f"{prefix}{connector}{size_tag}{entry.name}{star}{comment_str}")
