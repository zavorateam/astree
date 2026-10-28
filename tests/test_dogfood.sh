#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PYTHONPATH="${REPO_ROOT}/python/src:${PYTHONPATH:-}"

TEMP_DIR=$(mktemp -d)
trap 'rm -rf "${TEMP_DIR}"' EXIT

echo "=== Запуск сквозного Dogfood-теста Astree ==="

# 1. Дамп текущего репозитория через Python-движок
python3 -m astree.cli dump "${REPO_ROOT}" -L 2 > "${TEMP_DIR}/tree_py.txt"

# 2. Дамп через Rust-движок
cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- dump "${REPO_ROOT}" -L 2 > "${TEMP_DIR}/tree_rs.txt"

# 3. Проверка паритета вывода
if diff -u "${TEMP_DIR}/tree_py.txt" "${TEMP_DIR}/tree_rs.txt"; then
    echo "[PASS] Вывод дампа Python и Rust полностью идентичен!"
else
    echo "[FAIL] Расхождение между Python и Rust дампами!"
    exit 1
fi

# 4. Проверка разворачивания (dry-run)
python3 -m astree.cli "${TEMP_DIR}/tree_py.txt" -d >/dev/null
cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- "${TEMP_DIR}/tree_py.txt" -d >/dev/null
echo "[PASS] Dry-run разворачивание на обоих движках отработало корректно"

echo "=== Dogfood-тестирование успешно пройдено! ==="
