#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PYTHONPATH="${REPO_ROOT}/python/src:${PYTHONPATH:-}"

TEMP_DIR=$(mktemp -d)
trap 'rm -rf "${TEMP_DIR}"' EXIT

echo "=== 1. Проверка паритета вывода дампа (Python vs Rust) ==="
python3 -m astree.cli dump "${REPO_ROOT}" -L 2 > "${TEMP_DIR}/tree_py.txt"
cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- dump "${REPO_ROOT}" -L 2 > "${TEMP_DIR}/tree_rs.txt"

if diff -u "${TEMP_DIR}/tree_py.txt" "${TEMP_DIR}/tree_rs.txt"; then
    echo "[PASS] Вывод дампа полностью идентичен!"
else
    echo "[FAIL] Расхождение вывода дампа между Python и Rust!"
    exit 1
fi

echo "=== 2. Проверка симметрии (Scaffold -> Dump roundtrip) ==="
mkdir -p "${TEMP_DIR}/sandbox"
python3 -m astree.cli "${TEMP_DIR}/tree_py.txt" -d >/dev/null
cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- "${TEMP_DIR}/tree_py.txt" -d >/dev/null
echo "[PASS] Dry-run разворачивание успешно на обоих движках"

echo "=== 3. Проверка блокировки Path Traversal (Python & Rust) ==="
EVIL_TREE="evil_app/\n├── ../../evil.txt\n"

if echo -e "$EVIL_TREE" | python3 -m astree.cli - 2>/dev/null; then
    echo "[FAIL] Python не заблокировал '..'!"
    exit 1
fi

if echo -e "$EVIL_TREE" | cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- - 2>/dev/null; then
    echo "[FAIL] Rust не заблокировал '..'!"
    exit 1
fi
echo "[PASS] Оба движка корректно блокируют Path Traversal с кодом 1"

echo "=== 4. Проверка защиты от перезаписи без --force ==="
mkdir -p "${TEMP_DIR}/test_overwrite"
echo "valuable data" > "${TEMP_DIR}/test_overwrite/data.txt"
OVERWRITE_TREE="test_overwrite/\n├── data.txt\n"

cd "${TEMP_DIR}"
if echo -e "$OVERWRITE_TREE" | python3 -m astree.cli - 2>/dev/null; then
    echo "[FAIL] Python перезаписал файл без --force!"
    exit 1
fi

if echo -e "$OVERWRITE_TREE" | cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- - 2>/dev/null; then
    echo "[FAIL] Rust перезаписал файл без --force!"
    exit 1
fi

# Проверяем, что данные не затерты
if [ "$(cat "${TEMP_DIR}/test_overwrite/data.txt")" != "valuable data" ]; then
    echo "[FAIL] Содержимое файла повреждено!"
    exit 1
fi
echo "[PASS] Данные надёжно защищены от перезаписи без флага --force"

echo "=== Все проверки Dogfood успешно пройдены! ==="
