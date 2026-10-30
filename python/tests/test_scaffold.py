import os
import stat
import pytest
from pathlib import Path
from astree.scaffold import scaffold

def test_scaffold_creation(tmp_path):
    tree = """
    my_app/
    ├── README.md # my app description
    ├── scripts/
    │   └── run.py
    └── config.yaml # app config
    """
    scaffold(tree.strip().splitlines(), target_dir=tmp_path)

    assert (tmp_path / "my_app/README.md").exists()
    assert (tmp_path / "my_app/scripts/run.py").exists()

def test_overwrite_protection(tmp_path):
    tree = "app/\n├── file.txt # original\n"
    scaffold(tree.strip().splitlines(), target_dir=tmp_path)
    file_path = tmp_path / "app/file.txt"
    file_path.write_text("user data")

    with pytest.raises(FileExistsError):
        scaffold(tree.strip().splitlines(), target_dir=tmp_path, force=False)

    assert file_path.read_text() == "user data"

    scaffold(tree.strip().splitlines(), target_dir=tmp_path, force=True)
    assert file_path.read_text() != "user data"

def test_path_traversal_protection(tmp_path):
    tree = "app/\n├── ../../evil.txt\n"
    with pytest.raises(PermissionError):
        scaffold(tree.strip().splitlines(), target_dir=tmp_path)

def test_root_symlink_escape_protection(tmp_path):
    outside = tmp_path / "outside_dir"
    outside.mkdir()
    target_dir = tmp_path / "sandbox"
    target_dir.mkdir()

    os.symlink(outside, target_dir / "proj")

    tree = "proj/\n└── payload.sh\n"
    with pytest.raises(PermissionError):
        scaffold(tree.strip().splitlines(), target_dir=target_dir)

    assert not (outside / "payload.sh").exists()

def test_nested_symlink_escape_protection(tmp_path):
    outside = tmp_path / "outside_dir2"
    outside.mkdir()
    target_dir = tmp_path / "sandbox2"
    target_dir.mkdir()
    proj = target_dir / "proj"
    proj.mkdir()

    os.symlink(outside, proj / "sub")

    tree = "proj/\n└── sub/payload.sh\n"
    with pytest.raises(PermissionError):
        scaffold(tree.strip().splitlines(), target_dir=target_dir)

    assert not (outside / "payload.sh").exists()

def test_existing_real_directory_allowed(tmp_path):
    target_dir = tmp_path / "sandbox3"
    target_dir.mkdir()
    (target_dir / "proj").mkdir()

    tree = "proj/\n└── normal.txt\n"
    scaffold(tree.strip().splitlines(), target_dir=target_dir)
    assert (target_dir / "proj/normal.txt").exists()

def test_dry_run_leaves_filesystem_untouched(tmp_path):
    tree = "dry_proj/\n├── file1.txt\n└── sub/\n    └── file2.txt\n"
    scaffold(tree.strip().splitlines(), target_dir=tmp_path, dry_run=True)
    assert not (tmp_path / "dry_proj").exists()

# P3: Inline code executable-bit parity test
@pytest.mark.parametrize("rel_path,inline_str", [
    ("scripts/launch.py", 'print("hello")'),
    ("bin/tool.py", 'print("hello")'),
    ("cli/run.py", 'print("hello")'),
    ("main.py", 'print("hello")'),
    ("app.py", 'print("hello")'),
    ("script.bash", 'echo hello'),
    ("script.zsh", 'echo hello'),
])
def test_inline_code_executable_bit(tmp_path, rel_path, inline_str):
    target_dir = tmp_path / "p3_test"
    target_dir.mkdir()
    tree = f"proj/\n└── {rel_path} # = {inline_str}\n"
    scaffold(tree.strip().splitlines(), target_dir=target_dir)

    created = target_dir / "proj" / rel_path
    assert created.exists()
    assert created.read_text() == inline_str + "\n"
    assert not created.read_text().startswith("#!")
    mode = os.stat(created).st_mode
    assert mode & stat.S_IXUSR, f"{rel_path} must be executable"

# P4: Leaf symlink overwrite protection test
def test_leaf_symlink_overwrite_protection(tmp_path):
    outside = tmp_path / "outside_leaf.txt"
    outside.write_text("precious target")
    target_dir = tmp_path / "leaf_sandbox"
    target_dir.mkdir()
    proj = target_dir / "proj"
    proj.mkdir()

    link = proj / "link.txt"
    os.symlink(outside, link)

    tree = "proj/\n└── link.txt # = overwritten\n"

    # Without force -> must fail, target untouched
    with pytest.raises(FileExistsError):
        scaffold(tree.strip().splitlines(), target_dir=target_dir, force=False)
    assert outside.read_text() == "precious target"
    assert link.is_symlink()

    # With force -> must replace symlink with regular file, target untouched
    scaffold(tree.strip().splitlines(), target_dir=target_dir, force=True)
    assert outside.read_text() == "precious target"
    assert not link.is_symlink()
    assert link.is_file()
    assert link.read_text() == "overwritten\n"
