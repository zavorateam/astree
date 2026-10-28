import os
import stat
from astree.scaffold import scaffold

def test_scaffold_creation(tmp_path):
    tree = """
    my_app/
    ├── README.md # my app description
    ├── scripts/
    │   └── run.py
    └── config.yaml # app config
    """
    os.chdir(tmp_path)
    scaffold(tree.strip().splitlines())

    assert (tmp_path / "my_app/README.md").exists()
    assert (tmp_path / "my_app/scripts/run.py").exists()

    # Проверка shebang и прав исполнения для скрипта в scripts/
    run_file = tmp_path / "my_app/scripts/run.py"
    content = run_file.read_text()
    assert content.startswith("#!/usr/bin/env python3")
    assert os.stat(run_file).st_mode & stat.S_IXUSR
