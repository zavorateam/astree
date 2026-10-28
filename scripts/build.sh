#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="${REPO_ROOT}/build"
BASE_VERSION="0.2.0"

MODE="dev"
DO_INSTALL=false
PREFIX="${PREFIX:-}"
BINDIR_ARG="${BINDIR:-}"

# Парсинг аргументов
while [[ $# -gt 0 ]]; do
    case "$1" in
        prod) MODE="prod"; shift ;;
        dev)  MODE="dev"; shift ;;
        --install|-i) DO_INSTALL=true; shift ;;
        --prefix|-p)
            PREFIX="$2"; shift 2 ;;
        --prefix=*)
            PREFIX="${1#*=}"; shift ;;
        --bindir)
            BINDIR_ARG="$2"; shift 2 ;;
        --bindir=*)
            BINDIR_ARG="${1#*=}"; shift ;;
        --help|-h)
            echo "Использование: $0 [dev|prod] [--install|-i] [--prefix <dir>] [--bindir <dir>]"
            echo ""
            echo "Опции установки:"
            echo "  -i, --install         Установить бинарники в систему"
            echo "  -p, --prefix <dir>    Базовая директория (по умолчанию: ~/.local или /usr/local)"
            echo "  --bindir <dir>        Точная директория для исполняемых файлов (по умолчанию: <prefix>/bin)"
            exit 0
            ;;
        *)
            echo "Неизвестный аргумент: $1"
            exit 1
            ;;
    esac
done

# Определение директории установки
if [ "$DO_INSTALL" = true ]; then
    if [ -z "$PREFIX" ]; then
        if [ "$(id -u)" -eq 0 ]; then
            PREFIX="/usr/local"
        else
            PREFIX="$HOME/.local"
        fi
    fi
    BINDIR="${BINDIR_ARG:-${DESTDIR:-}${PREFIX}/bin}"
fi

# Версионирование
if [ "$MODE" = "prod" ]; then
    VERSION="${BASE_VERSION}"
    RUST_FLAGS="--release"
    RUST_SUBDIR="release"
    echo "=== [BUILD: PROD] Версия: ${VERSION} ==="
else
    GIT_HASH=$(git -C "${REPO_ROOT}" rev-parse --short HEAD 2>/dev/null || echo "local")
    VERSION="${BASE_VERSION}-dev+${GIT_HASH}"
    RUST_FLAGS=""
    RUST_SUBDIR="debug"
    echo "=== [BUILD: DEV] Версия: ${VERSION} ==="
fi

mkdir -p "${BUILD_DIR}"

# 1. Сборка Rust
echo "-> Сборка Rust бинарника..."
cargo build ${RUST_FLAGS} --manifest-path "${REPO_ROOT}/rust/Cargo.toml"
BIN_SRC="${REPO_ROOT}/target/${RUST_SUBDIR}/astree"
[ ! -f "${BIN_SRC}" ] && BIN_SRC="${REPO_ROOT}/rust/target/${RUST_SUBDIR}/astree"

cp "${BIN_SRC}" "${BUILD_DIR}/astree"
chmod 755 "${BUILD_DIR}/astree"
echo "  [OK] Собран: ${BUILD_DIR}/astree"

# 2. Установка
if [ "$DO_INSTALL" = true ]; then
    echo "-> Установка бинарников в ${BINDIR} (prefix: ${PREFIX})..."
    install -d "${BINDIR}"
    
    # Установка Python пакета (создает astree-py в <prefix>/bin)
    python3 -m pip install "${REPO_ROOT}" --prefix="${PREFIX}" --no-deps --force-reinstall --quiet
    
    # Установка нативного Rust бинарника как главного 'astree'
    install -m 755 "${BUILD_DIR}/astree" "${BINDIR}/astree"
    
    echo "  [OK] ${BINDIR}/astree    (быстрый нативный Rust бинарник)"
    echo "  [OK] ${BINDIR}/astree-py (Python CLI)"
    echo "Установка успешно завершена в ${BINDIR}!"
fi
