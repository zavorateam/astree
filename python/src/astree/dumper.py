import math
import os
import re
from pathlib import Path

BINARY_EXTS = {
    '.o', '.a', '.so', '.dylib', '.dll', '.exe', '.bin', '.lock',
    '.pyc', '.pyd', '.wasm', '.png', '.jpg', '.jpeg', '.gif', '.ico',
    '.pdf', '.zip', '.tar', '.gz', '.bz2', '.xz', '.7z', '.rlib',
    '.d', '.pdb', '.timestamp'
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

def human_size(n_bytes: int) -> str:
    """P6: Единое округление human-readable размеров (round-half-up / away-from-zero)."""
    units = ['B', 'K', 'M', 'G', 'T', 'P']
    val = float(n_bytes)
    for i, unit in enumerate(units):
        if val < 1024.0 or i == len(units) - 1:
            if i == 0:
                return f"[{n_bytes:>4}B]"
            if val < 9.95:
                rounded = math.floor(val * 10.0 + 0.5) / 10.0
                return f"[{rounded:>4.1f}{unit}]"
            rounded_int = int(math.floor(val + 0.5))
            return f"[{rounded_int:>4}{unit}]"
        val /= 1024.0
    return f"[{val:>4.1f}P]"

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

def glob_to_regex(pat: str) -> re.Pattern:
    """P5: Трансляция glob-шаблона в Regex, где '*' не пересекает '/'."""
    re_str = ["^"]
    i = 0
    n = len(pat)
    while i < n:
        c = pat[i]
        if c == "*":
            if i + 1 < n and pat[i + 1] == "*":
                i += 2
                if i < n and pat[i] == "/":
                    i += 1
                    re_str.append("(?:.+/)?")
                else:
                    re_str.append(".*")
            else:
                i += 1
                re_str.append("[^/]*")
        elif c == "?":
            i += 1
            re_str.append("[^/]")
        elif c in r".+()[]{}\|^$":
            re_str.append("\\" + c)
            i += 1
        else:
            re_str.append(c)
            i += 1
    re_str.append("$")
    return re.compile("".join(re_str))

class GitIgnoreRules:
    def __init__(self, root: Path):
        self.rules: list[tuple[bool, bool, bool, re.Pattern]] = []
        gi = root / ".gitignore"
        if gi.is_file():
            try:
                with open(gi, "r", encoding="utf-8", errors="ignore") as f:
                    for line in f:
                        line = line.strip()
                        if not line or line.startswith("#"):
                            continue
                        is_neg = False
                        if line.startswith("!"):
                            is_neg = True
                            line = line[1:].strip()
                        if not line:
                            continue
                        is_dir_only = line.endswith("/")
                        pat = line.strip("/")
                        has_slash = "/" in pat
                        regex = glob_to_regex(pat)
                        self.rules.append((is_neg, is_dir_only, has_slash, regex))
            except Exception:
                pass

    def is_ignored(self, name: str, rel_path: str, is_dir: bool) -> bool:
        ignored = False
        for is_neg, is_dir_only, has_slash, regex in self.rules:
            if is_dir_only and not is_dir:
                continue
            matched = False
            if has_slash:
                if regex.match(rel_path):
                    matched = True
            else:
                if regex.match(name) or regex.match(rel_path):
                    matched = True
            if matched:
                ignored = not is_neg
        return ignored

def dump_tree(root: Path, current: Path, prefix="", level=1, max_level=None,
              show_sizes=False, dirs_only=False, ignore_pat=None, gi_rules=None, out=None):
    if max_level is not None and level > max_level:
        return

    if gi_rules is None:
        gi_rules = GitIgnoreRules(root)

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
        if gi_rules.is_ignored(name, rel, entry.is_dir()):
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
                      show_sizes, dirs_only, ignore_pat, gi_rules, out=out)
        else:
            is_exec = os.access(entry, os.X_OK) and not entry.is_dir()
            star = "*" if is_exec else ""
            comment = extract_comment(entry)
            comment_str = f" # {comment}" if comment else ""
            writer(f"{prefix}{connector}{size_tag}{entry.name}{star}{comment_str}\n" if out else f"{prefix}{connector}{size_tag}{entry.name}{star}{comment_str}")
