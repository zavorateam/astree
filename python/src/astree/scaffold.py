import os
import stat
import tempfile
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

def validate_safe_path(base_dir: Path, rel_path: str, boundary: Path | None = None) -> Path:
    """Защита от Path Traversal: блокирует абсолютные пути, '..' и запись через внешние симлинки."""
    p = Path(rel_path)
    if p.is_absolute():
        raise PermissionError(f"Security violation: absolute path '{rel_path}' is prohibited")
    if ".." in p.parts:
        raise PermissionError(f"Security violation: parent traversal '..' in '{rel_path}' is prohibited")

    effective_boundary = (boundary or base_dir).resolve()
    
    # Поэтапная проверка всех промежуточных директорий на побег через симлинки за границу проекта
    curr = base_dir
    for part in p.parts[:-1]:
        curr = curr / part
        if curr.is_symlink():
            resolved_curr = curr.resolve()
            if not resolved_curr.is_relative_to(effective_boundary):
                raise PermissionError(
                    f"Security violation: intermediate symlink '{curr}' escapes boundary '{effective_boundary}'"
                )

    return base_dir / p

def generate_content(path: Path, comment: str | None, inline_code: str | None,
                     project_name: str, is_exec: bool, no_shebang: bool, no_templates: bool) -> tuple[str, bool]:
    ext = path.suffix.lower()
    fname = path.name
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

    parts = path.parts
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

def safe_write_file(path: Path, content: str, is_exec: bool, force: bool):
    """Атомарная запись без TOCTOU-гонок и с разрывом существующих симлинков."""
    parent = path.parent
    parent.mkdir(parents=True, exist_ok=True)

    if not force:
        try:
            with open(path, "x", encoding="utf-8") as f:
                f.write(content)
        except FileExistsError:
            raise FileExistsError(f"File or symlink '{path}' already exists. Use --force to overwrite")
    else:
        if path.exists() and path.is_dir() and not path.is_symlink():
            raise IsADirectoryError(f"Cannot overwrite directory '{path}' with a file")

        with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=parent, delete=False) as tf:
            tf.write(content)
            temp_name = tf.name

        os.replace(temp_name, path)

    if is_exec:
        current_mode = os.stat(path).st_mode
        os.chmod(path, current_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)

def scaffold(lines: list[str], target_dir: Path = Path("."), dry_run=False, force=False, no_shebang=False, no_templates=False):
    stack = []
    base_dir = target_dir.resolve()
    root_name = target_dir.name or "project"
    project_boundary = base_dir

    # Определяем, имеет ли дерево корневую папку нулевого уровня
    has_root = False
    for raw in lines:
        node = parse_line(raw)
        if node:
            if node.depth == 0 and node.is_dir:
                has_root = True
            break

    for raw in lines:
        node = parse_line(raw)
        if not node:
            continue

        effective_depth = node.depth if has_root else max(0, node.depth - 1)
        stack = stack[:effective_depth]
        rel_path = os.path.join(*stack, node.name) if stack else node.name
        current_path = validate_safe_path(base_dir, rel_path, boundary=project_boundary)

        if node.is_dir:
            stack.append(node.name)
            if node.depth == 0 and has_root:
                root_name = node.name
                project_boundary = (base_dir / node.name).resolve()
            if not dry_run:
                if current_path.is_file():
                    raise FileExistsError(f"Cannot create directory '{current_path}': file exists")
                current_path.mkdir(parents=True, exist_ok=True)
            print(f"[DIR]  {rel_path}/")
            continue

        if node.symlink_target:
            if not dry_run:
                if current_path.is_symlink() or current_path.exists():
                    if not force:
                        raise FileExistsError(f"Cannot overwrite '{current_path}' with symlink without --force")
                    if current_path.is_dir() and not current_path.is_symlink():
                        raise IsADirectoryError(f"Cannot overwrite directory '{current_path}' with symlink")
                    current_path.unlink()
                current_path.parent.mkdir(parents=True, exist_ok=True)
                os.symlink(node.symlink_target, current_path)
            print(f"[LINK] {rel_path} -> {node.symlink_target}")
            continue

        content, make_exec = generate_content(
            current_path, node.comment, node.inline_code, root_name,
            node.is_exec, no_shebang, no_templates
        )

        if not dry_run:
            safe_write_file(current_path, content, make_exec, force)

        print(f"[FILE] {rel_path}{' (+x)' if make_exec else ''}")
