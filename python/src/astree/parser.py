import re
from dataclasses import dataclass
from typing import Optional

@dataclass
class ParsedNode:
    depth: int
    name: str
    is_dir: bool
    is_exec: bool
    symlink_target: Optional[str] = None
    comment: Optional[str] = None
    inline_code: Optional[str] = None

def strip_metadata_prefixes(line: str) -> str:
    """Удаляет метаданные размеров из вывода tree -h (например, [ 21K] или [ 716])."""
    return re.sub(r'(\s*[│├└─\s]+)\[\s*[\d\.]+[BKMGT]?\s*\]\s+', r'\1', line)

def clean_input_lines(raw_text: str) -> list[str]:
    """Извлекает блок дерева из Markdown (```...```) и очищает вывод."""
    m = re.search(r'```(?:[a-zA-Z0-9_\-]+)?\n(.*?)\n```', raw_text, re.DOTALL)
    if m:
        raw_text = m.group(1)

    lines = []
    for line in raw_text.splitlines():
        line = strip_metadata_prefixes(line)
        stripped = line.strip()
        if not stripped:
            continue
        if re.search(r'[│├└─]', line) or stripped.endswith('/'):
            lines.append(line)
    return lines

def parse_line(raw_line: str) -> Optional[ParsedNode]:
    line = raw_line.rstrip()
    if not line:
        return None

    # Отделяем префикс дерева (каждый уровень — 4 символа)
    m_prefix = re.match(r'^(?:[│\s]*[├└]──\s*|\s*)', line)
    prefix = m_prefix.group(0) if m_prefix else ''
    body = line[len(prefix):].strip()
    if not body:
        return None

    depth = len(prefix) // 4 if prefix else 0

    # Выделяем инлайн-комментарий (# или //)
    comment = None
    inline_code = None
    m_comment = re.search(r'\s+(?:#|//)\s*(.*)$', body)
    if m_comment:
        c_text = m_comment.group(1).strip()
        body = body[:m_comment.start()].strip()
        if c_text.startswith('='):
            inline_code = c_text[1:].strip().replace(r'\n', '\n')
        else:
            comment = c_text

    # Симлинки: link -> target
    symlink_target = None
    if "->" in body:
        parts = body.split("->", 1)
        body = parts[0].strip()
        symlink_target = parts[1].strip()

    # Флаг исполняемого файла (*)
    is_exec = False
    if body.endswith('*'):
        is_exec = True
        body = body[:-1]

    is_dir = body.endswith('/')
    if is_dir:
        body = body.rstrip('/')

    return ParsedNode(
        depth=depth,
        name=body,
        is_dir=is_dir,
        is_exec=is_exec,
        symlink_target=symlink_target,
        comment=comment,
        inline_code=inline_code,
    )
