import argparse
import shutil
import subprocess
import sys
from pathlib import Path
from astree import __version__
from astree.parser import clean_input_lines
from astree.scaffold import scaffold
from astree.dumper import dump_tree

def get_clipboard() -> str:
    for cmd in [["pbpaste"], ["wl-paste"], ["xclip", "-selection", "clipboard", "-o"]]:
        if shutil.which(cmd[0]):
            return subprocess.check_output(cmd, text=True)
    sys.exit("Ошибка: утилита буфера обмена (pbpaste, wl-paste, xclip) не найдена.")

def main():
    # Режим DUMP
    if len(sys.argv) > 1 and sys.argv[1] == "dump":
        dump_p = argparse.ArgumentParser(
            prog="astree dump",
            add_help=False,
            description="Дамп структуры проекта с комментариями."
        )
        dump_p.add_argument("--help", action="help", help="Показать справку")
        dump_p.add_argument("dir", nargs="?", default=".", help="Директория (по умолчанию .)")
        dump_p.add_argument("-h", "--human-readable", action="store_true", help="Показывать размеры (как tree -h)")
        dump_p.add_argument("-L", "--level", type=int, default=None, help="Глубина обхода")
        dump_p.add_argument("-I", "--ignore", help="Regex-паттерн игнорируемых файлов")
        dump_p.add_argument("-d", "--dirs-only", action="store_true", help="Только директории")
        dump_p.add_argument("-o", "--output", help="Вывод в файл")

        args = dump_p.parse_args(sys.argv[2:])
        root = Path(args.dir).resolve()

        if args.output:
            sys.stdout = open(args.output, "w", encoding="utf-8")

        print(f"{root.name}/")
        dump_tree(root, root, level=1, max_level=args.level,
                  show_sizes=args.human_readable, dirs_only=args.dirs_only,
                  ignore_pat=args.ignore)
        return

    # Режим SCAFFOLD
    parser = argparse.ArgumentParser(prog="astree", description="Разворачивание структуры дерева в реальные файлы.")
    parser.add_argument("file", nargs="?", default=None, help="Файл со структурой дерева (или stdin)")
    parser.add_argument("-c", "--clip", action="store_true", help="Взять из буфера обмена")
    parser.add_argument("-d", "--dry-run", action="store_true", help="Превью без создания файлов")
    parser.add_argument("-f", "--force", action="store_true", help="Перезаписывать существующие файлы")
    parser.add_argument("--no-shebang", action="store_true", help="Не проставлять шебанги")
    parser.add_argument("--no-templates", action="store_true", help="Не генерировать шаблоны")
    parser.add_argument("-V", "--version", action="version", version=f"astree v{__version__}")

    args = parser.parse_args()

    if args.clip:
        raw_text = get_clipboard()
    elif args.file and args.file != "-":
        with open(args.file, "r", encoding="utf-8") as f:
            raw_text = f.read()
    else:
        if sys.stdin.isatty():
            parser.print_help()
            sys.exit(0)
        raw_text = sys.stdin.read()

    lines = clean_input_lines(raw_text)
    scaffold(lines, dry_run=args.dry_run, force=args.force,
             no_shebang=args.no_shebang, no_templates=args.no_templates)

if __name__ == "__main__":
    main()
