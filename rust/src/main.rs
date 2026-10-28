use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process;
use regex::Regex;

mod parser;
mod scaffold;
mod dumper;

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
                    println!("Usage: astree dump [DIR] [-L level] [-I pattern] [-h] [-d] [-o file]");
                    return;
                }
                other => {
                    if !other.starts_with('-') { dir = PathBuf::from(other); }
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

        let mut writer: Box<dyn Write> = match output_file {
            Some(path) => match File::create(&path) {
                Ok(f) => Box::new(f),
                Err(e) => {
                    eprintln!("Error: Cannot create output file {:?}: {}", path, e);
                    process::exit(1);
                }
            },
            None => Box::new(io::stdout()),
        };

        let _ = writeln!(writer, "{}/", root.file_name().unwrap_or_default().to_string_lossy());
        dumper::dump_dir(&root, &root, "", 1, max_level, show_sizes, dirs_only, ignore_re.as_ref(), &mut writer);
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
        println!("Options:\n  -d, --dry-run          Preview actions without touching disk");
        println!("  -f, --force            Safely overwrite existing files");
        println!("  --no-shebang           Skip shebang injection");
        println!("  --no-templates         Skip template expansion");
        return;
    }

    let mut dry_run = false;
    let mut force = false;
    let mut no_shebang = false;
    let mut no_templates = false;
    let mut file_path = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
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

    let mut input = String::new();
    if let Some(path) = file_path {
        if path != "-" {
            input = match fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error: Cannot read file '{}': {}", path, e);
                    process::exit(1);
                }
            };
        } else {
            io::stdin().read_to_string(&mut input).unwrap();
        }
    } else {
        io::stdin().read_to_string(&mut input).unwrap();
    }

    if let Err(e) = scaffold::scaffold(&input, &PathBuf::from("."), dry_run, force, no_shebang, no_templates) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
