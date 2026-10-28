# 🌳 astree

[![CI](https://github.com/zavorateam/astree/actions/workflows/ci.yml/badge.svg)](https://github.com/zavorateam/astree/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Python 3.10+](https://img.shields.io/badge/python-3.10+-blue.svg)](https://www.python.org/downloads/)
[![Rust 2021](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)

**Astree** — безопасный двусторонний CLI-инструмент для преобразования текстовых представлений деревьев каталогов в реальные файловые структуры и обратно.

Разработан как универсальный мост между человеком, большими языковыми моделями (LLM) и файловой системой:
1. **Tree &rarr; Repo (Scaffold):** превращает схемы каталогов из чата, документации или Markdown-блоков в рабочий проект с правильными шебангами, правами исполнения и шаблонами.
2. **Repo &rarr; Tree (Dump):** извлекает компактную схему репозитория с описаниями файлов из docstring и заголовков, соблюдая `.gitignore` и фильтруя бинарный шум.

Доступен в виде **высокопроизводительного нативного бинарника на Rust** (`astree`) и **Python-пакета** (`astree-py`) с гарантированным побайтовым паритетом поведения.

---

## ⚡ Быстрый старт

### Развернуть проект из схемы дерева
```bash
astree << 'EOF'
my_service/
├── pyproject.toml              # автоматически получит шаблон PEP 621
├── README.md                   # Core service documentation
├── .gitignore                  # базовые исключения для Python/Rust
├── scripts/
│   └── migrate.py*             # получит shebang #!/usr/bin/env python3 и chmod +x
└── src/
    ├── __init__.py             # = __version__ = "0.1.0"
    └── server.py               # service entrypoint
