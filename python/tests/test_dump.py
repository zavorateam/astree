import io
from pathlib import Path
from astree.dumper import is_binary, extract_comment, human_size, dump_tree

def test_binary_guard(tmp_path):
    bin_file = tmp_path / "test.o"
    bin_file.write_bytes(b"\x7fELF\x02\x01\x01\x00# not a comment\x00")
    assert is_binary(bin_file) is True
    assert extract_comment(bin_file) == ""

def test_rust_attribute_not_comment(tmp_path):
    rs_file = tmp_path / "main.rs"
    rs_file.write_text('#[cfg(unix)]\nuse std::os::unix;\n//! Real module doc\n')
    assert extract_comment(rs_file) == "Real module doc"

# P6: Human size rounding parity tests
def test_human_size_rounding():
    assert human_size(0) == "[   0B]"
    assert human_size(500) == "[ 500B]"
    assert human_size(1023) == "[1023B]"
    assert human_size(1024) == "[ 1.0K]"
    assert human_size(1536) == "[ 1.5K]"
    assert human_size(10751) == "[  10K]"
    assert human_size(10752) == "[  11K]"  # 10.5 KiB boundary
    assert human_size(10753) == "[  11K]"
    assert human_size(1048576) == "[ 1.0M]"

# P5: .gitignore matching parity tests
def test_gitignore_nested_and_negation(tmp_path):
    root = tmp_path / "gi_repo"
    root.mkdir()
    (root / ".gitignore").write_text("*.log\n!important.log\nsrc/*.py\nbuild/\n")

    (root / "src").mkdir()
    (root / "src/sub").mkdir()
    (root / "build").mkdir()

    (root / "app.log").write_text("log")
    (root / "important.log").write_text("keep")
    (root / "src/main.py").write_text("main")
    (root / "src/sub/deep.py").write_text("deep")
    (root / "build/artifact.o").write_text("build")
    (root / "README.md").write_text("readme")

    buf = io.StringIO()
    dump_tree(root, root, out=buf)
    output = buf.getvalue()

    assert "app.log" not in output
    assert "important.log" in output
    assert "main.py" not in output
    assert "deep.py" in output  # src/*.py does NOT match src/sub/deep.py
    assert "build" not in output
    assert "README.md" in output
