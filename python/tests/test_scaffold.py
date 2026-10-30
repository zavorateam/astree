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
    """РЕГРЕССИОННЫЙ ТЕСТ: корень дерева proj/ является внешним симлинком."""
    outside = tmp_path / "outside_dir"
    outside.mkdir()
    target_dir = tmp_path / "sandbox"
    target_dir.mkdir()

    # target_dir/proj указывает на outside_dir
    import os
    os.symlink(outside, target_dir / "proj")

    tree = "proj/\n└── payload.sh\n"
    with pytest.raises(PermissionError):
        scaffold(tree.strip().splitlines(), target_dir=target_dir)

    assert not (outside / "payload.sh").exists()

def test_nested_symlink_escape_protection(tmp_path):
    """РЕГРЕССИОННЫЙ ТЕСТ: вложенный симлинк внутри реального корня указывает наружу."""
    outside = tmp_path / "outside_dir2"
    outside.mkdir()
    target_dir = tmp_path / "sandbox2"
    target_dir.mkdir()
    proj = target_dir / "proj"
    proj.mkdir()

    import os
    os.symlink(outside, proj / "sub")

    tree = "proj/\n└── sub/payload.sh\n"
    with pytest.raises(PermissionError):
        scaffold(tree.strip().splitlines(), target_dir=target_dir)

    assert not (outside / "payload.sh").exists()

def test_existing_real_directory_allowed(tmp_path):
    """Разрешённый сценарий: корневой каталог уже реально существует на диске."""
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
