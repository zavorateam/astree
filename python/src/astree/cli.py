import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path
from astree import __version__
from astree.parser import clean_input_lines
from astree.scaffold import scaffold
from astree.dumper import dump_tree

def get_clipboard() -> str:
    for cmd in [["pbpaste"], ["wl-paste"], ["xclip", "-selection", "clipboard", "-o"], ["xsel", "--clipboard", "--output"]]:
        if shutil.which(cmd[0]):
            try:
                return subprocess.check_output(cmd, text=True)
            except Exception:
                pass
    sys.stderr.write("Error: no clipboard utility available (pbpaste, wl-paste, xclip, xsel)\n")
    sys.exit(1)

def main():
    if len(sys.argv) > 1 and sys.argv[1] == "dump":
        dump_p = argparse.ArgumentParser(
            prog="astree dump",
            add_help=False,
            description="Dump directory tree with comments and metadata."
        )
        dump_p.add_argument("--help", action="help", help="Show this help message")
        dump_p.add_argument("dir", nargs="?", default=".", help="Directory to dump (default: .)")
        dump_p.add_argument("-h", "--human-readable", action="store_true", help="Print human readable file sizes")
        dump_p.add_argument("-L", "--level", type=int, default=None, help="Max display depth (>= 0)")
        dump_p.add_argument("-I", "--ignore", help="Regex pattern of files/directories to ignore")
        dump_p.add_argument("-d", "--dirs-only", action="store_true", help="List directories only")
        dump_p.add_argument("-o", "--output", help="Output to file instead of stdout")

        args = dump_p.parse_args(sys.argv[2:])
        if args.level is not None and args.level < 0:
            sys.stderr.write("Error: -L/--level must be non-negative\n")
            sys.exit(1)

        if args.ignore:
            try:
                re.compile(args.ignore)
            except re.error as e:
                sys.stderr.write(f"Error: Invalid regex for -I/--ignore: {e}\n")
                sys.exit(1)

        root = Path(args.dir).resolve()
        if not root.is_dir():
            sys.stderr.write(f"Error: '{args.dir}' is not a directory\n")
            sys.exit(1)

        out_stream = None
        if args.output:
            try:
                out_stream = open(args.output, "w", encoding="utf-8")
            except Exception as e:
                sys.stderr.write(f"Error: Cannot open output file '{args.output}': {e}\n")
                sys.exit(1)

        header = f"{root.name}/\n" if out_stream else f"{root.name}/"
        if out_stream:
            out_stream.write(header)
        else:
            print(header)

        try:
            dump_tree(root, root, level=1, max_level=args.level,
                      show_sizes=args.human_readable, dirs_only=args.dirs_only,
                      ignore_pat=args.ignore, out=out_stream)
        except Exception as e:
            sys.stderr.write(f"Error during dump: {e}\n")
            sys.exit(1)
        finally:
            if out_stream:
                out_stream.close()
        return

    parser = argparse.ArgumentParser(
        prog="astree",
        description="Turn text file tree representations into real filesystems."
    )
    parser.add_argument("file", nargs="?", default=None, help="Input tree file or '-' for stdin")
    parser.add_argument("-t", "--target-dir", default=".", help="Target directory for scaffolding (default: .)")
    parser.add_argument("-c", "--clip", action="store_true", help="Read tree from clipboard")
    parser.add_argument("-d", "--dry-run", action="store_true", help="Preview actions without writing to disk")
    parser.add_argument("-f", "--force", action="store_true", help="Safely overwrite existing files")
    parser.add_argument("--no-shebang", action="store_true", help="Do not inject shebang lines")
    parser.add_argument("--no-templates", action="store_true", help="Do not generate template boilerplates")
    parser.add_argument("-V", "--version", action="version", version=f"astree v{__version__}")

    args = parser.parse_args()

    if args.clip:
        raw_text = get_clipboard()
    elif args.file and args.file != "-":
        p = Path(args.file)
        if not p.is_file():
            sys.stderr.write(f"Error: File '{args.file}' not found\n")
            sys.exit(1)
        raw_text = p.read_text(encoding="utf-8")
    else:
        if sys.stdin.isatty():
            parser.print_help()
            sys.exit(0)
        raw_text = sys.stdin.read()

    lines = clean_input_lines(raw_text)
    try:
        target_path = Path(args.target_dir)
        scaffold(lines, target_dir=target_path, dry_run=args.dry_run, force=args.force,
                 no_shebang=args.no_shebang, no_templates=args.no_templates)
    except Exception as e:
        sys.stderr.write(f"Error: {e}\n")
        sys.exit(1)

if __name__ == "__main__":
    main()
