from astree.dumper import is_binary, extract_comment

def test_binary_guard(tmp_path):
    bin_file = tmp_path / "test.o"
    bin_file.write_bytes(b"\x7fELF\x02\x01\x01\x00# not a comment\x00")
    assert is_binary(bin_file) is True
    assert extract_comment(bin_file) == ""

def test_rust_attribute_not_comment(tmp_path):
    rs_file = tmp_path / "main.rs"
    rs_file.write_text('#[cfg(unix)]\nuse std::os::unix;\n//! Real module doc\n')
    # Должен игнорировать #[cfg(unix)] и находить //! Real module doc
    assert extract_comment(rs_file) == "Real module doc"
