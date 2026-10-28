from astree.parser import parse_line, strip_metadata_prefixes, clean_input_lines

def test_strip_metadata():
    line = "├── [ 21K]  README.md"
    assert strip_metadata_prefixes(line) == "├── README.md"

def test_parse_node():
    node = parse_line("│   ├── train.py* # main runner")
    assert node.depth == 2
    assert node.name == "train.py"
    assert node.is_exec is True
    assert node.comment == "main runner"

def test_inline_code():
    node = parse_line("├── version.py # = __version__ = \"1.0.0\"")
    assert node.inline_code == '__version__ = "1.0.0"'

def test_symlink():
    node = parse_line("└── link.py -> ../orig.py")
    assert node.symlink_target == "../orig.py"
