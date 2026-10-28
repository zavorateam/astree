#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PYTHONPATH="${REPO_ROOT}/python/src:${PYTHONPATH:-}"

RUST_BIN="${REPO_ROOT}/target/debug/astree"
if [ ! -f "${RUST_BIN}" ]; then
    cargo build --manifest-path "${REPO_ROOT}/rust/Cargo.toml"
fi

SANDBOX=$(mktemp -d)
trap 'rm -rf "${SANDBOX}"' EXIT

echo "=== 1. Проверка паритета дампа репозитория (Python vs Rust) ==="
python3 -m astree.cli dump "${REPO_ROOT}" -L 2 > "${SANDBOX}/dump_py.txt"
"${RUST_BIN}" dump "${REPO_ROOT}" -L 2 > "${SANDBOX}/dump_rs.txt"

if diff -u "${SANDBOX}/dump_py.txt" "${SANDBOX}/dump_rs.txt"; then
    echo "[PASS] Вывод дампа полностью идентичен!"
else
    echo "[FAIL] Расхождение вывода дампа между Python и Rust!"
    exit 1
fi

echo "=== 2. Проверка симметрии разворачивания (Scaffold parity через -t) ==="
mkdir -p "${SANDBOX}/py_scaffold" "${SANDBOX}/rs_scaffold"

# Разворачиваем структуру обоими движками через штатный флаг -t
python3 -m astree.cli "${SANDBOX}/dump_py.txt" -t "${SANDBOX}/py_scaffold"
"${RUST_BIN}" "${SANDBOX}/dump_rs.txt" -t "${SANDBOX}/rs_scaffold"

# Снимаем дамп с обоих развёрнутых проектов и сравниваем
python3 -m astree.cli dump "${SANDBOX}/py_scaffold" > "${SANDBOX}/res_py.txt"
"${RUST_BIN}" dump "${SANDBOX}/rs_scaffold" > "${SANDBOX}/res_rs.txt"

tail -n +2 "${SANDBOX}/res_py.txt" > "${SANDBOX}/body_py.txt"
tail -n +2 "${SANDBOX}/res_rs.txt" > "${SANDBOX}/body_rs.txt"

if diff -u "${SANDBOX}/body_py.txt" "${SANDBOX}/body_rs.txt"; then
    echo "[PASS] Развёрнутые структуры обоих движков идентичны!"
else
    echo "[FAIL] Развёрнутые структуры различаются!"
    exit 1
fi

echo "=== 3. Проверка безопасности: блокировка Path Traversal ==="
cd "${SANDBOX}"
EVIL_TREE="evil_app/\n├── ../../evil.txt\n"

if echo -e "$EVIL_TREE" | python3 -m astree.cli - 2>/dev/null; then
    echo "[FAIL] Python не заблокировал относительный путь '..'!"
    exit 1
fi

if echo -e "$EVIL_TREE" | "${RUST_BIN}" - 2>/dev/null; then
    echo "[FAIL] Rust не заблокировал относительный путь '..'!"
    exit 1
fi

if [ -f "${SANDBOX}/evil.txt" ] || [ -f "${REPO_ROOT}/evil.txt" ]; then
    echo "[FAIL] Файл был создан за пределами корня проекта!"
    exit 1
fi
echo "[PASS] Оба движка блокируют Path Traversal без утечки файлов"

echo "=== 4. Проверка защиты от перезаписи без --force ==="
mkdir -p "${SANDBOX}/guard_test"
echo "sensitive data" > "${SANDBOX}/guard_test/file.txt"
OVERWRITE_TREE="guard_test/\n├── file.txt\n"

if echo -e "$OVERWRITE_TREE" | python3 -m astree.cli - 2>/dev/null; then
    echo "[FAIL] Python перезаписал существующий файл без --force!"
    exit 1
fi

if echo -e "$OVERWRITE_TREE" | "${RUST_BIN}" - 2>/dev/null; then
    echo "[FAIL] Rust перезаписал существующий файл без --force!"
    exit 1
fi

if [ "$(cat "${SANDBOX}/guard_test/file.txt")" != "sensitive data" ]; then
    echo "[FAIL] Исходные данные были перезаписаны!"
    exit 1
fi
echo "[PASS] Данные надёжно защищены от перезаписи без флага --force"

echo "=== Все фазы Dogfood-тестирования завершены успешно! ==="
