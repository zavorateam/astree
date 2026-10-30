
#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="${REPO_ROOT}/build"

BASE_VERSION="0.2.3"
PYTHON_BIN="${PYTHON_BIN:-python3}"

MODE="dev"
DO_INSTALL=false
PREFIX="${PREFIX:-}"
BINDIR_ARG="${BINDIR:-}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        prod) MODE="prod"; shift ;;
        dev) MODE="dev"; shift ;;
        --install|-i) DO_INSTALL=true; shift ;;
        --prefix|-p)
            [[ $# -ge 2 ]] || { echo "Missing value for --prefix" >&2; exit 1; }
            PREFIX="$2"
            shift 2
            ;;
        --prefix=*)
            PREFIX="${1#*=}"
            shift
            ;;
        --bindir)
            [[ $# -ge 2 ]] || { echo "Missing value for --bindir" >&2; exit 1; }
            BINDIR_ARG="$2"
            shift 2
            ;;
        --bindir=*)
            BINDIR_ARG="${1#*=}"
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [dev|prod] [--install|-i] [--prefix <dir>] [--bindir <dir>]"
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: Cargo is not installed or not in PATH." >&2
    exit 1
fi

if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
    echo "ERROR: Python executable '$PYTHON_BIN' not found." >&2
    exit 1
fi

if ! "$PYTHON_BIN" -c 'import PyInstaller' >/dev/null 2>&1; then
    echo "ERROR: PyInstaller is not installed for $PYTHON_BIN." >&2
    echo "Install it with: $PYTHON_BIN -m pip install PyInstaller" >&2
    exit 1
fi

if [[ "$MODE" == "prod" ]]; then
    VERSION="${BASE_VERSION}"
    RUST_BUILD_ARGS=(--release)
    RUST_SUBDIR="release"
    PYINSTALLER_OPTIMIZE=2
    echo "=== [BUILD: PROD] Version: ${VERSION} ==="
else
    GIT_HASH="$(git -C "${REPO_ROOT}" rev-parse --short HEAD 2>/dev/null || echo local)"
    VERSION="${BASE_VERSION}-dev+${GIT_HASH}"
    RUST_BUILD_ARGS=()
    RUST_SUBDIR="debug"
    PYINSTALLER_OPTIMIZE=0
    echo "=== [BUILD: DEV] Version: ${VERSION} ==="
fi

# Determine executable suffix for Windows.
EXE_SUFFIX=""
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
        EXE_SUFFIX=".exe"
        ;;
esac

RUST_EXECUTABLE="astree${EXE_SUFFIX}"
PYTHON_EXECUTABLE="astree-py${EXE_SUFFIX}"

# Linux-only stripping. Avoid modifying macOS binaries and Windows executables.
PYINSTALLER_STRIP=()
case "$(uname -s)" in
    Linux*)
        PYINSTALLER_STRIP+=(--strip)
        ;;
esac

PYTHON_VERSION="$(
    "$PYTHON_BIN" -c '
import sys
sys.path.insert(0, sys.argv[1])
from astree import __version__
print(__version__)
' "${REPO_ROOT}/python/src"
)"

if [[ "$PYTHON_VERSION" != "$BASE_VERSION" ]]; then
    echo "ERROR: Python version mismatch." >&2
    echo "Expected: ${BASE_VERSION}" >&2
    echo "Found:    ${PYTHON_VERSION}" >&2
    exit 1
fi

mkdir -p "${BUILD_DIR}"

# 1. Build native Rust executable.
echo "-> Building Rust executable..."

cargo build \
    "${RUST_BUILD_ARGS[@]}" \
    --manifest-path "${REPO_ROOT}/rust/Cargo.toml"

RUST_BIN_SRC="${REPO_ROOT}/target/${RUST_SUBDIR}/${RUST_EXECUTABLE}"

if [[ ! -f "$RUST_BIN_SRC" ]]; then
    RUST_BIN_SRC="${REPO_ROOT}/rust/target/${RUST_SUBDIR}/${RUST_EXECUTABLE}"
fi

if [[ ! -f "$RUST_BIN_SRC" ]]; then
    echo "ERROR: Rust executable not found: ${RUST_EXECUTABLE}" >&2
    exit 1
fi

cp "$RUST_BIN_SRC" "${BUILD_DIR}/${RUST_EXECUTABLE}"
chmod 755 "${BUILD_DIR}/${RUST_EXECUTABLE}"

echo "  [OK] ${BUILD_DIR}/${RUST_EXECUTABLE}"

# 2. Build standalone Python executable using PyInstaller.
echo "-> Building standalone Python executable..."

PYINSTALLER_DIST="${BUILD_DIR}/pyinstaller-dist"
PYINSTALLER_WORK="${BUILD_DIR}/pyinstaller-work"
PYINSTALLER_SPEC="${BUILD_DIR}/pyinstaller-spec"

rm -rf "$PYINSTALLER_DIST" "$PYINSTALLER_WORK" "$PYINSTALLER_SPEC"

mkdir -p "$PYINSTALLER_DIST" "$PYINSTALLER_WORK" "$PYINSTALLER_SPEC"

PYINSTALLER_ARGS=(
    --clean
    --noconfirm
    --onefile
    --optimize "$PYINSTALLER_OPTIMIZE"
    --name astree-py
    --paths "${REPO_ROOT}/python/src"
    --distpath "$PYINSTALLER_DIST"
    --workpath "$PYINSTALLER_WORK"
    --specpath "$PYINSTALLER_SPEC"
)

PYINSTALLER_ARGS+=("${PYINSTALLER_STRIP[@]}")

"$PYTHON_BIN" -m PyInstaller \
    "${PYINSTALLER_ARGS[@]}" \
    "${REPO_ROOT}/python/src/astree/cli.py"

PYTHON_BIN_SRC="${PYINSTALLER_DIST}/${PYTHON_EXECUTABLE}"

if [[ ! -f "$PYTHON_BIN_SRC" ]]; then
    echo "ERROR: PyInstaller executable not found: ${PYTHON_BIN_SRC}" >&2
    exit 1
fi

cp "$PYTHON_BIN_SRC" "${BUILD_DIR}/${PYTHON_EXECUTABLE}"
chmod 755 "${BUILD_DIR}/${PYTHON_EXECUTABLE}"

echo "  [OK] ${BUILD_DIR}/${PYTHON_EXECUTABLE}"

# 3. Smoke tests.
echo "-> Running executable smoke tests..."

RUST_VERSION="$("${BUILD_DIR}/${RUST_EXECUTABLE}" --version)"
PYTHON_VERSION_OUTPUT="$("${BUILD_DIR}/${PYTHON_EXECUTABLE}" --version)"

if [[ "$RUST_VERSION" != "astree v${BASE_VERSION}" ]]; then
    echo "ERROR: Unexpected Rust version: ${RUST_VERSION}" >&2
    exit 1
fi

if [[ "$PYTHON_VERSION_OUTPUT" != "astree v${BASE_VERSION}" ]]; then
    echo "ERROR: Unexpected Python version: ${PYTHON_VERSION_OUTPUT}" >&2
    exit 1
fi

"${BUILD_DIR}/${RUST_EXECUTABLE}" --help >/dev/null
"${BUILD_DIR}/${PYTHON_EXECUTABLE}" --help >/dev/null

echo "  [OK] Rust:   ${RUST_VERSION}"
echo "  [OK] Python: ${PYTHON_VERSION_OUTPUT}"

# 4. Installation.
if [[ "$DO_INSTALL" == true ]]; then
    if [[ -z "$PREFIX" ]]; then
        if [[ "$(id -u)" -eq 0 ]]; then
            PREFIX="/usr/local"
        else
            PREFIX="${HOME}/.local"
        fi
    fi

    BINDIR="${BINDIR_ARG:-${DESTDIR:-}${PREFIX}/bin}"

    echo "-> Installing executables into ${BINDIR}..."
    install -d "${BINDIR}"

    install -m 755 \
        "${BUILD_DIR}/${RUST_EXECUTABLE}" \
        "${BINDIR}/${RUST_EXECUTABLE}"

    install -m 755 \
        "${BUILD_DIR}/${PYTHON_EXECUTABLE}" \
        "${BINDIR}/${PYTHON_EXECUTABLE}"

    echo "  [OK] ${BINDIR}/${RUST_EXECUTABLE} (native Rust)"
    echo "  [OK] ${BINDIR}/${PYTHON_EXECUTABLE} (standalone Python)"
    echo "Installation completed."
fi

echo
echo "=== BUILD SUCCESSFUL ==="
ls -lh \
    "${BUILD_DIR}/${RUST_EXECUTABLE}" \
    "${BUILD_DIR}/${PYTHON_EXECUTABLE}"
