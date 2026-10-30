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

echo "=== 3. Проверка P3: Executable bit на inline code (Python & Rust) ==="
mkdir -p "${SANDBOX}/p3_py" "${SANDBOX}/p3_rs"
P3_TREE="p3_proj/\n├── scripts/launch.py # = print('hello')\n├── bin/tool.py # = print('hello')\n├── cli/run.py # = print('hello')\n├── main.py # = print('hello')\n├── app.py # = print('hello')\n├── script.bash # = echo hello\n└── script.zsh # = echo hello\n"

printf "%b" "$P3_TREE" | python3 -m astree.cli -t "${SANDBOX}/p3_py" - >/dev/null
printf "%b" "$P3_TREE" | "${RUST_BIN}" -t "${SANDBOX}/p3_rs" - >/dev/null

for f in scripts/launch.py bin/tool.py cli/run.py main.py app.py script.bash script.zsh; do
    if [ ! -x "${SANDBOX}/p3_py/p3_proj/$f" ]; then
        echo "[FAIL] Python: $f не получил права на исполнение!"
        exit 1
    fi
    if [ ! -x "${SANDBOX}/p3_rs/p3_proj/$f" ]; then
        echo "[FAIL] Rust: $f не получил права на исполнение!"
        exit 1
    fi
    # Проверка, что в inline code нет неожиданного shebang
    if grep -q "^#!" "${SANDBOX}/p3_py/p3_proj/$f" || grep -q "^#!" "${SANDBOX}/p3_rs/p3_proj/$f"; then
        echo "[FAIL] Shebang был ошибочно добавлен в inline code ($f)!"
        exit 1
    fi
done
echo "[PASS] P3: Права на исполнение и отсутствие shebang подтверждены на всех 7 сценариях"

echo "=== 4. Проверка P4: Leaf symlink с --force и без (Python & Rust) ==="
mkdir -p "${SANDBOX}/p4_py" "${SANDBOX}/p4_rs"
OUT_PY="${SANDBOX}/p4_py/outside.txt"
OUT_RS="${SANDBOX}/p4_rs/outside.txt"
echo "original py" > "${OUT_PY}"
echo "original rs" > "${OUT_RS}"

mkdir -p "${SANDBOX}/p4_py/proj" "${SANDBOX}/p4_rs/proj"
ln -s "${OUT_PY}" "${SANDBOX}/p4_py/proj/link.txt"
ln -s "${OUT_RS}" "${SANDBOX}/p4_rs/proj/link.txt"

P4_TREE="proj/\n└── link.txt # = overwritten\n"

# 4.1 Без --force -> отказ, цель не тронута
if printf "%b" "$P4_TREE" | python3 -m astree.cli -t "${SANDBOX}/p4_py" - 2>/dev/null; then
    echo "[FAIL] Python перезаписал leaf symlink без --force!"
    exit 1
fi
if printf "%b" "$P4_TREE" | "${RUST_BIN}" -t "${SANDBOX}/p4_rs" - 2>/dev/null; then
    echo "[FAIL] Rust перезаписал leaf symlink без --force!"
    exit 1
fi

if [ "$(cat "${OUT_PY}")" != "original py" ] || [ "$(cat "${OUT_RS}")" != "original rs" ]; then
    echo "[FAIL] Исходная цель симлинка была изменена без --force!"
    exit 1
fi

# 4.2 С --force -> ссылка заменена, цель не тронута
printf "%b" "$P4_TREE" | python3 -m astree.cli -t "${SANDBOX}/p4_py" -f - >/dev/null
printf "%b" "$P4_TREE" | "${RUST_BIN}" -t "${SANDBOX}/p4_rs" -f - >/dev/null

if [ "$(cat "${OUT_PY}")" != "original py" ] || [ "$(cat "${OUT_RS}")" != "original rs" ]; then
    echo "[FAIL] Исходная цель симлинка была изменена даже при --force!"
    exit 1
fi
if [ -L "${SANDBOX}/p4_py/proj/link.txt" ] || [ -L "${SANDBOX}/p4_rs/proj/link.txt" ]; then
    echo "[FAIL] Симлинк не был разорван при --force!"
    exit 1
fi
echo "[PASS] P4: Защита конечных симлинков полностью подтверждена для Python и Rust"

echo "=== 5. Проверка P5: .gitignore с вложенными путями и инверсией ==="
mkdir -p "${SANDBOX}/gi_test"
cat << 'GI_EOF' > "${SANDBOX}/gi_test/.gitignore"
*.log
!important.log
src/*.py
build/
GI_EOF

mkdir -p "${SANDBOX}/gi_test/src" "${SANDBOX}/gi_test/src/sub" "${SANDBOX}/gi_test/build"
echo "log" > "${SANDBOX}/gi_test/app.log"
echo "important" > "${SANDBOX}/gi_test/important.log"
echo "main" > "${SANDBOX}/gi_test/src/main.py"
echo "deep" > "${SANDBOX}/gi_test/src/sub/deep.py"
echo "build" > "${SANDBOX}/gi_test/build/artifact.o"
echo "readme" > "${SANDBOX}/gi_test/README.md"

python3 -m astree.cli dump "${SANDBOX}/gi_test" > "${SANDBOX}/gi_py.txt"
"${RUST_BIN}" dump "${SANDBOX}/gi_test" > "${SANDBOX}/gi_rs.txt"

if ! diff -u "${SANDBOX}/gi_py.txt" "${SANDBOX}/gi_rs.txt"; then
    echo "[FAIL] P5: Расхождение вывода .gitignore между Python и Rust!"
    exit 1
fi

if grep -q "app.log" "${SANDBOX}/gi_py.txt" || ! grep -q "important.log" "${SANDBOX}/gi_py.txt"; then
    echo "[FAIL] P5: Некорректная обработка *.log или !important.log!"
    exit 1
fi
if grep -q "main.py" "${SANDBOX}/gi_py.txt" || ! grep -q "deep.py" "${SANDBOX}/gi_py.txt"; then
    echo "[FAIL] P5: src/*.py ошибочно захватил вложенный каталог src/sub/deep.py!"
    exit 1
fi
echo "[PASS] P5: Идентичное поведение .gitignore (включая src/*.py и !important.log) подтверждено"

echo "=== 6. Проверка P6: Граничные размеры 10751, 10752 (10.5 KiB), 10753 байт ==="
mkdir -p "${SANDBOX}/p6_test"
head -c 10751 </dev/zero > "${SANDBOX}/p6_test/f_10751.bin"
head -c 10752 </dev/zero > "${SANDBOX}/p6_test/f_10752.bin"
head -c 10753 </dev/zero > "${SANDBOX}/p6_test/f_10753.bin"

python3 -m astree.cli dump "${SANDBOX}/p6_test" -h > "${SANDBOX}/p6_py.txt"
"${RUST_BIN}" dump "${SANDBOX}/p6_test" -h > "${SANDBOX}/p6_rs.txt"

if ! diff -u "${SANDBOX}/p6_py.txt" "${SANDBOX}/p6_rs.txt"; then
    echo "[FAIL] P6: Расхождение форматирования размеров между Python и Rust!"
    exit 1
fi

if ! grep -q "\[  10K\]  f_10751.bin" "${SANDBOX}/p6_py.txt"; then
    echo "[FAIL] 10751 байт должен округляться до [  10K]!"
    exit 1
fi
if ! grep -q "\[  11K\]  f_10752.bin" "${SANDBOX}/p6_py.txt"; then
    echo "[FAIL] 10752 байта (10.5 KiB) должен округляться до [  11K]!"
    exit 1
fi
if ! grep -q "\[  11K\]  f_10753.bin" "${SANDBOX}/p6_py.txt"; then
    echo "[FAIL] 10753 байта должен округляться до [  11K]!"
    exit 1
fi
echo "[PASS] P6: Побайтовое совпадение вывода на границе округления 10.5 KiB подтверждено"

echo "=== Все проверки E2E Dogfood успешно завершены! ==="
