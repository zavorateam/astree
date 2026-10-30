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

python3 -m astree.cli "${SANDBOX}/dump_py.txt" -t "${SANDBOX}/py_scaffold"
"${RUST_BIN}" "${SANDBOX}/dump_rs.txt" -t "${SANDBOX}/rs_scaffold"

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

echo "=== 3. Проверка безопасности: корневой симлинк (Root Symlink Escape) ==="
OUTSIDE_ROOT="${SANDBOX}/outside_root"
mkdir -p "${OUTSIDE_ROOT}"
mkdir -p "${SANDBOX}/root_escape_py" "${SANDBOX}/root_escape_rs"

ln -s "${OUTSIDE_ROOT}" "${SANDBOX}/root_escape_py/proj"
ln -s "${OUTSIDE_ROOT}" "${SANDBOX}/root_escape_rs/proj"

TREE_ROOT_ESCAPE="proj/\n└── root_payload.sh\n"

if printf "%b" "$TREE_ROOT_ESCAPE" | python3 -m astree.cli -t "${SANDBOX}/root_escape_py" - 2>/dev/null; then
    echo "[FAIL] Python разрешил побег через корневой симлинк!"
    exit 1
fi

if printf "%b" "$TREE_ROOT_ESCAPE" | "${RUST_BIN}" -t "${SANDBOX}/root_escape_rs" - 2>/dev/null; then
    echo "[FAIL] Rust разрешил побег через корневой симлинк!"
    exit 1
fi

if [ -f "${OUTSIDE_ROOT}/root_payload.sh" ]; then
    echo "[FAIL] root_payload.sh был создан во внешней директории!"
    exit 1
fi
echo "[PASS] Оба движка блокируют побег через корневой симлинк"

echo "=== 4. Проверка безопасности: конечный симлинк (Leaf Symlink Hijacking) ==="
mkdir -p "${SANDBOX}/leaf_test"
OUTSIDE_TARGET="${SANDBOX}/outside_leaf_file.txt"
echo "precious original" > "${OUTSIDE_TARGET}"
ln -s "${OUTSIDE_TARGET}" "${SANDBOX}/leaf_test/link.txt"

LEAF_TREE="leaf_test/\n├── link.txt # = overwritten\n"

if printf "%b" "$LEAF_TREE" | python3 -m astree.cli -t "${SANDBOX}" - 2>/dev/null; then
    echo "[FAIL] Python перезаписал симлинк без --force!"
    exit 1
fi
if printf "%b" "$LEAF_TREE" | "${RUST_BIN}" -t "${SANDBOX}" - 2>/dev/null; then
    echo "[FAIL] Rust перезаписал симлинк без --force!"
    exit 1
fi

if [ "$(cat "${OUTSIDE_TARGET}")" != "precious original" ]; then
    echo "[FAIL] Файл-мишень симлинка был изменён без --force!"
    exit 1
fi

printf "%b" "$LEAF_TREE" | "${RUST_BIN}" -t "${SANDBOX}" -f - >/dev/null
if [ "$(cat "${OUTSIDE_TARGET}")" != "precious original" ]; then
    echo "[FAIL] Файл-мишень симлинка был изменён даже при --force!"
    exit 1
fi
if [ -L "${SANDBOX}/leaf_test/link.txt" ]; then
    echo "[FAIL] Симлинк не был разорван при --force!"
    exit 1
fi
echo "[PASS] Защита от Symlink Hijacking подтверждена (мишень не затронута)"

echo "=== 5. Проверка сохранения скобок [notes] и путей с пробелами ==="
mkdir -p "${SANDBOX}/spaces_test"
SPACE_TREE="spaces_proj/\n├── [notes] file.txt\n├── my script.py*\n└── sub folder/\n    └── spaced file.txt\n"

printf "%b" "$SPACE_TREE" | "${RUST_BIN}" -t "${SANDBOX}/spaces_test" - >/dev/null

if [ ! -f "${SANDBOX}/spaces_test/spaces_proj/[notes] file.txt" ]; then
    echo "[FAIL] Файл [notes] file.txt был искажён!"
    exit 1
fi
if [ ! -x "${SANDBOX}/spaces_test/spaces_proj/my script.py" ]; then
    echo "[FAIL] my script.py не получил права на исполнение!"
    exit 1
fi
echo "[PASS] Имена в скобках [notes] и пути с пробелами обработаны корректно"

echo "=== Все проверки E2E Dogfood успешно завершены! ==="
