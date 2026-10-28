#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="${REPO_ROOT}/build"
BASE_VERSION="0.2.0"

MODE="dev"
DO_INSTALL=false

# Парсинг POSIX-аргументов
for arg in "$@"; do
    case "$arg" in
        prod) MODE="prod" ;;
        dev)  MODE="dev" ;;
        --install|-i) DO_INSTALL=true ;;
        --help)
            echo "Usage: $0 [dev|prod] [--install|-i]"
            exit 0
            ;;
    esac
done

# Определение префикса установки
if [ "$DO_INSTALL" = true ]; then
    if [ "$(id -u)" -eq 0 ]; then
        PREFIX="${PREFIX:-/usr/local}"
    else
        PREFIX="${PREFIX:-$HOME/.local}"
    fi
    BINDIR="${DESTDIR:-}${PREFIX}/bin"
fi

# Определение версий
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

# 1. Сборка Python (build/astree-py)
echo "-> Сборка Python (astree-py)..."
cat << PYEOF > "${BUILD_DIR}/astree-py"
#!/usr/bin/env python3
import sys, os
__version__ = "${VERSION}"
if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] in ("--version", "-V"):
        print(f"astree-py version {__version__} (${MODE})")
        sys.exit(0)
    src_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "python", "src"))
    if os.path.exists(src_dir) and src_dir not in sys.path:
        sys.path.insert(0, src_dir)
    try:
        from astree.cli import main
        main()
    except ImportError:
        print(f"astree-py {__version__} (python-модули в разработке)")
PYEOF
chmod 755 "${BUILD_DIR}/astree-py"
echo "  [OK] Создан: build/astree-py"

# 2. Сборка Rust (build/astree)
echo "-> Сборка Rust (astree)..."
if command -v cargo >/dev/null 2>&1; then
    sed -i.bak "s/^version = .*/version = \"${BASE_VERSION}\"/" "${REPO_ROOT}/rust/Cargo.toml" && rm -f "${REPO_ROOT}/rust/Cargo.toml.bak"
    cargo build ${RUST_FLAGS} --manifest-path "${REPO_ROOT}/rust/Cargo.toml"

    BIN_SRC="${REPO_ROOT}/target/${RUST_SUBDIR}/astree"
    [ ! -f "${BIN_SRC}" ] && BIN_SRC="${REPO_ROOT}/rust/target/${RUST_SUBDIR}/astree"

    cp "${BIN_SRC}" "${BUILD_DIR}/astree"
    chmod 755 "${BUILD_DIR}/astree"
    echo "  [OK] Создан: build/astree"
else
    echo "  [!] Cargo не найден. Пропуск Rust."
fi

# 3. Установка в систему (--install)
if [ "$DO_INSTALL" = true ]; then
    echo "-> Установка в ${BINDIR}..."
    install -d "${BINDIR}"
    if [ -f "${BUILD_DIR}/astree" ]; then
        install -m 755 "${BUILD_DIR}/astree" "${BINDIR}/astree"
        echo "  [OK] Установлен: ${BINDIR}/astree"
    fi
    install -m 755 "${BUILD_DIR}/astree-py" "${BINDIR}/astree-py"
    echo "  [OK] Установлен: ${BINDIR}/astree-py"
    echo "Установка завершена! Проверьте: which astree"
fi
