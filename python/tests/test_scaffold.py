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
    file_path.write_text("user modified data")

    # Без --force должна выбрасываться ошибка
    with pytest.raises(FileExistsError):
        scaffold(tree.strip().splitlines(), target_dir=tmp_path, force=False)

    assert file_path.read_text() == "user modified data"

    # С --force файл успешно перезаписывается
    scaffold(tree.strip().splitlines(), target_dir=tmp_path, force=True)
    assert file_path.read_text() != "user modified data"

def test_path_traversal_protection(tmp_path):
    tree = "app/\n├── ../../evil.txt\n"
    with pytest.raises(PermissionError):
        scaffold(tree.strip().splitlines(), target_dir=tmp_path)
