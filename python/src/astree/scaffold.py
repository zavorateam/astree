import os
import stat
from pathlib import Path
from astree.parser import parse_line

TEMPLATES = {
    "pyproject": """[project]
name = "{project_name}"
version = "0.1.0"
description = "{comment}"
readme = "README.md"
requires-python = ">=3.10"
dependencies = []

[build-system]
requires = ["setuptools>=61.0"]
build-backend = "setuptools.build_meta"
""",
    "readme": """# {project_name}

> {comment}
""",
    "gitignore": """__pycache__/
*.py[cod]
.venv/
build/
dist/
target/
*.egg-info/
.env
.DS_Store
"""
}

LANGUAGE_RULES = {
    ".py": {"shebang": "#!/usr/bin/env python3", "style": "docstring"},
    ".sh": {"shebang": "#!/usr/bin/env bash", "style": "hash"},
    ".bash": {"shebang": "#!/usr/bin/env bash", "style": "hash"},
    ".zsh": {"shebang": "#!/usr/bin/env zsh", "style": "hash"},
    ".js": {"shebang": "#!/usr/bin/env node", "style": "slash"},
    ".ts": {"shebang": None, "style": "slash"},
    ".rs": {"shebang": None, "style": "rustdoc"},
    ".go": {"shebang": None, "style": "slash"},
    ".yaml": {"shebang": None, "style": "hash"},
    ".yml": {"shebang": None, "style": "hash"},
    ".toml": {"shebang": None, "style": "hash"},
    ".md": {"shebang": None, "style": "md"},
    ".json": {"shebang": None, "style": "json"},
}

SCRIPT_DIRS = {"scripts", "bin", "cli"}

def generate_content(path: str, comment: str | None, inline_code: str | None,
                     project_name: str, is_exec: bool, no_shebang: bool, no_templates: bool) -> tuple[str, bool]:
    ext = Path(path).suffix.lower()
    fname = os.path.basename(path)
    rule = LANGUAGE_RULES.get(ext, {"shebang": None, "style": "hash"})

    if inline_code is not None:
        return inline_code + "\n", is_exec

    if not no_templates:
        if fname == "pyproject.toml":
            return TEMPLATES["pyproject"].format(project_name=project_name, comment=comment or ""), False
        if fname == "README.md" and comment:
            return TEMPLATES["readme"].format(project_name=project_name, comment=comment), False
        if fname == ".gitignore":
            return TEMPLATES["gitignore"], False

    parts = Path(path).parts
    should_shebang = False
    if not no_shebang:
        if is_exec:
            should_shebang = True
        elif ext in {".sh", ".bash", ".zsh"}:
            should_shebang = True
            is_exec = True
        elif ext == ".py" and (any(d in SCRIPT_DIRS for d in parts) or fname in {"main.py", "app.py"}):
            should_shebang = True
            is_exec = True

    lines = []
    if should_shebang and rule["shebang"]:
        lines.append(rule["shebang"])

    if comment:
        style = rule["style"]
        if style == "docstring":
            lines.append(f'"""{comment}"""')
        elif style == "rustdoc":
            lines.append(f'//! {comment}')
        elif style == "slash":
            lines.append(f'// {comment}')
        elif style == "json":
            pass
        else:
            lines.append(f'# {comment}')

    content = ("\n".join(lines) + "\n") if lines else ""
    return content, is_exec

def scaffold(lines: list[str], dry_run=False, force=False, no_shebang=False, no_templates=False):
    stack = []
    root_name = "project"

    for raw in lines:
        node = parse_line(raw)
        if not node:
            continue

        stack = stack[:node.depth]
        current_path = os.path.join(*stack, node.name) if stack else node.name

        if node.is_dir:
            stack.append(node.name)
            if node.depth == 0:
                root_name = node.name
            if not dry_run:
                os.makedirs(current_path, exist_ok=True)
            print(f"[DIR]  {current_path}")
            continue

        if node.symlink_target:
            if not dry_run:
                parent = os.path.dirname(current_path)
                if parent:
                    os.makedirs(parent, exist_ok=True)
                if (os.path.exists(current_path) or os.path.islink(current_path)) and force:
                    os.unlink(current_path)
                os.symlink(node.symlink_target, current_path)
            print(f"[LINK] {current_path} -> {node.symlink_target}")
            continue

        content, make_exec = generate_content(
            current_path, node.comment, node.inline_code, root_name,
            node.is_exec, no_shebang, no_templates
        )

        if not dry_run:
            parent = os.path.dirname(current_path)
            if parent:
                os.makedirs(parent, exist_ok=True)
            with open(current_path, "w", encoding="utf-8") as f:
                f.write(content)
            if make_exec:
                os.chmod(current_path, os.stat(current_path).st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)

        print(f"[FILE] {current_path}{' (+x)' if make_exec else ''}")
