import io
import sys
from astree.dumper import is_binary, extract_comment, dump_tree

def test_binary_guard(tmp_path):
    bin_file = tmp_path / "test.o"
    bin_file.write_bytes(b"\x7fELF\x02\x01\x01\x00# not a comment\x00")
    assert is_binary(bin_file) is True
    assert extract_comment(bin_file) == ""

def test_comment_extraction(tmp_path):
    py_file = tmp_path / "mod.py"
    py_file.write_text('"""Important module docstring"""\n')
    assert extract_comment(py_file) == "Important module docstring"
