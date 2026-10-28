import io
from pathlib import Path
from astree.dumper import dump_tree
from astree.scaffold import scaffold
from astree.parser import clean_input_lines

def test_roundtrip_structural_equivalence(tmp_path):
    src = tmp_path / "src_repo"
    src.mkdir()
    (src / "sub").mkdir()
    (src / "sub/mod.py").write_text('"""Module doc"""\n')
    (src / "README.md").write_text('# Title\n')

    # 1. Дамп исходного каталога с корневым заголовком
    buf1 = io.StringIO()
    buf1.write(f"{src.name}/\n")
    dump_tree(src, src, out=buf1)
    tree1 = buf1.getvalue()

    # 2. Разворачивание структуры в чистый каталог
    dst = tmp_path / "dst_repo"
    dst.mkdir()
    lines = clean_input_lines(tree1)
    scaffold(lines, target_dir=dst)

    # 3. Дамп нового каталога
    buf2 = io.StringIO()
    buf2.write(f"{src.name}/\n")
    dump_tree(dst / "src_repo", dst / "src_repo", out=buf2)
    tree2 = buf2.getvalue()

    assert tree1 == tree2
