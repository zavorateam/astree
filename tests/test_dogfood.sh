#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PYTHONPATH="${REPO_ROOT}/python/src:${PYTHONPATH:-}"

SANDBOX=$(mktemp -d)
trap 'rm -rf "${SANDBOX}"' EXIT

echo "=== 1. Проверка паритета дампа репозитория (Python vs Rust) ==="
python3 -m astree.cli dump "${REPO_ROOT}" -L 2 > "${SANDBOX}/dump_py.txt"
cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- dump "${REPO_ROOT}" -L 2 > "${SANDBOX}/dump_rs.txt"

if diff -u "${SANDBOX}/dump_py.txt" "${SANDBOX}/dump_rs.txt"; then
    echo "[PASS] Вывод дампа полностью идентичен!"
else
    echo "[FAIL] Расхождение вывода дампа между Python и Rust!"
    exit 1
fi

echo "=== 2. Проверка симметрии разворачивания (Scaffold parity) ==="
mkdir -p "${SANDBOX}/py_scaffold" "${SANDBOX}/rs_scaffold"

# Разворачиваем структуру обоими движками в независимые песочницы
python3 -m astree.cli "${SANDBOX}/dump_py.txt" --target-dir "${SANDBOX}/py_scaffold" 2>/dev/null || (
    cd "${SANDBOX}/py_scaffold" && python3 -m astree.cli "${SANDBOX}/dump_py.txt" >/dev/null
)
(
    cd "${SANDBOX}/rs_scaffold" && cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- "${SANDBOX}/dump_rs.txt" >/dev/null
)

# Снимаем дамп с обоих развёрнутых проектов и сравниваем
python3 -m astree.cli dump "${SANDBOX}/py_scaffold" > "${SANDBOX}/res_py.txt"
cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- dump "${SANDBOX}/rs_scaffold" > "${SANDBOX}/res_rs.txt"

# Имена корневых директорий в дампе различаются (py_scaffold vs rs_scaffold), убираем первую строку для сравнения содержимого
tail -n +2 "${SANDBOX}/res_py.txt" > "${SANDBOX}/body_py.txt"
tail -n +2 "${SANDBOX}/res_rs.txt" > "${SANDBOX}/body_rs.txt"

if diff -u "${SANDBOX}/body_py.txt" "${SANDBOX}/body_rs.txt"; then
    echo "[PASS] Развёрнутые структуры обоих движков идентичны!"
else
    echo "[FAIL] Развёрнутые структуры различаются!"
    exit 1
fi

echo "=== 3. Проверка безопасности: блокировка Path Traversal в изолированной среде ==="
cd "${SANDBOX}"
EVIL_TREE="evil_app/\n├── ../../evil.txt\n"

if echo -e "$EVIL_TREE" | python3 -m astree.cli - 2>/dev/null; then
    echo "[FAIL] Python не заблокировал относительный путь '..'!"
    exit 1
fi

if echo -e "$EVIL_TREE" | cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- - 2>/dev/null; then
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

if echo -e "$OVERWRITE_TREE" | cargo run --quiet --manifest-path "${REPO_ROOT}/rust/Cargo.toml" -- - 2>/dev/null; then
    echo "[FAIL] Rust перезаписал существующий файл без --force!"
    exit 1
fi

if [ "$(cat "${SANDBOX}/guard_test/file.txt")" != "sensitive data" ]; then
    echo "[FAIL] Исходные данные были перезаписаны!"
    exit 1
fi
echo "[PASS] Данные надёжно защищены от перезаписи без флага --force"

echo "=== Все фазы Dogfood-тестирования завершены успешно! ==="
