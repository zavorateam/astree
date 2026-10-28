use regex::Regex;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{self, Command};

mod dumper;
mod parser;
mod scaffold;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const CLIPBOARD_COMMANDS: &[&[&str]] = &[
    &["pbpaste"],
    &["wl-paste"],
    &["xclip", "-selection", "clipboard", "-o"],
    &["xsel", "--clipboard", "--output"],
];

fn get_clipboard() -> io::Result<String> {
    for tool in CLIPBOARD_COMMANDS {
        if let Some((cmd, args)) = tool.split_first() {
            if let Ok(output) = Command::new(cmd).args(args).output() {
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                }
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "No clipboard utility available (pbpaste, wl-paste, xclip, xsel)",
    ))
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "dump" {
        let mut dir = PathBuf::from(".");
        let mut show_sizes = false;
        let mut dirs_only = false;
        let mut max_level = None;
        let mut ignore_re = None;
        let mut output_file = None;

        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--human-readable" => show_sizes = true,
                "-d" | "--dirs-only" => dirs_only = true,
                "-L" | "--level" => {
                    i += 1;
                    if i < args.len() {
                        match args[i].parse::<usize>() {
                            Ok(lvl) => max_level = Some(lvl),
                            Err(_) => {
                                eprintln!("Error: -L/--level must be a non-negative integer");
                                process::exit(1);
                            }
                        }
                    }
                }
                "-I" | "--ignore" => {
                    i += 1;
                    if i < args.len() {
                        match Regex::new(&args[i]) {
                            Ok(re) => ignore_re = Some(re),
                            Err(e) => {
                                eprintln!("Error: Invalid regex for -I: {}", e);
                                process::exit(1);
                            }
                        }
                    }
                }
                "-o" | "--output" => {
                    i += 1;
                    if i < args.len() {
                        output_file = Some(PathBuf::from(&args[i]));
                    }
                }
                "--help" => {
                    println!(
                        "Usage: astree dump [DIR] [-L level] [-I pattern] [-h] [-d] [-o file]"
                    );
                    return;
                }
                other => {
                    if !other.starts_with('-') {
                        dir = PathBuf::from(other);
                    }
                }
            }
            i += 1;
        }

        let root = match dir.canonicalize() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: Cannot resolve directory {:?}: {}", dir, e);
                process::exit(1);
            }
        };

        if !root.is_dir() {
            eprintln!("Error: {:?} is not a directory", root);
            process::exit(1);
        }

        let mut writer: Box<dyn io::Write> = match output_file {
            Some(path) => match File::create(&path) {
                Ok(f) => Box::new(f),
                Err(e) => {
                    eprintln!("Error: Cannot create output file {:?}: {}", path, e);
                    process::exit(1);
                }
            },
            None => Box::new(io::stdout()),
        };

        if let Err(e) = writeln!(
            writer,
            "{}/",
            root.file_name().unwrap_or_default().to_string_lossy()
        ) {
            eprintln!("Error writing output: {}", e);
            process::exit(1);
        }

        let options = dumper::DumpOptions {
            max_level,
            show_sizes,
            dirs_only,
            ignore_re: ignore_re.as_ref(),
        };

        if let Err(e) = dumper::dump_dir(&root, &root, "", 1, &options, &mut writer) {
            eprintln!("Error during dump: {}", e);
            process::exit(1);
        }
        return;
    }

    if args.len() > 1 && (args[1] == "--version" || args[1] == "-V") {
        println!("astree v{}", VERSION);
        return;
    }

    if args.len() > 1 && (args[1] == "--help") {
        println!("astree - smart tree to repo generator & dumper\n");
        println!("Usage:\n  astree [FILE]          Scaffold tree into filesystem");
        println!("  astree dump [DIR]      Dump filesystem into annotated tree\n");
        println!("Options:\n  -c, --clip             Read tree from clipboard");
        println!("  -d, --dry-run          Preview actions without touching disk");
        println!("  -f, --force            Safely overwrite existing files");
        println!("  --no-shebang           Skip shebang injection");
        println!("  --no-templates         Skip template expansion");
        return;
    }

    let mut dry_run = false;
    let mut force = false;
    let mut no_shebang = false;
    let mut no_templates = false;
    let mut clip = false;
    let mut file_path = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-c" | "--clip" => clip = true,
            "-d" | "--dry-run" => dry_run = true,
            "-f" | "--force" => force = true,
            "--no-shebang" => no_shebang = true,
            "--no-templates" => no_templates = true,
            other => {
                if !other.starts_with('-') && file_path.is_none() {
                    file_path = Some(other.to_string());
                }
            }
        }
        i += 1;
    }

    let input = if clip {
        match get_clipboard() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
    } else if let Some(path) = file_path {
        if path != "-" {
            match fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: Cannot read file '{}': {}", path, e);
                    process::exit(1);
                }
            }
        } else {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf).unwrap();
            buf
        }
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).unwrap();
        buf
    };

    let lines = parser::clean_input_lines(&input);
    if let Err(e) = scaffold::scaffold(
        &lines,
        &PathBuf::from("."),
        dry_run,
        force,
        no_shebang,
        no_templates,
    ) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
