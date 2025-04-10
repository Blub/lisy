use std::io;
use std::os::unix::ffi::OsStrExt;

use lisy::fs::Stat;

fn usage(mut out: impl io::Write, status: i32) -> ! {
    let _ = writeln!(
        out,
        "\
usage: ismount [options...] path
options:
  -v, --verbose         verbose output
"
    );

    std::process::exit(status);
}

fn main() {
    let mut args = std::env::args_os().skip(1);

    let mut verbose = false;
    let mut source = None;

    for arg_os in args.by_ref() {
        let arg = arg_os.as_bytes();

        if arg == b"-h" || arg == b"--help" {
            usage(std::io::stdout(), 0);
        }

        if arg == b"-v" || arg == b"--vebose" {
            verbose = true;
            continue;
        }

        if arg == b"--" {
            break;
        } else if arg.starts_with(b"-") {
            usage(std::io::stderr(), 1);
        } else {
            source = Some(arg_os);
            break;
        };
    }

    let source = match source.or_else(|| args.next()) {
        Some(p) => p,
        None => usage(std::io::stderr(), 1),
    };

    let meta = match Stat::new_empty().stat(&source) {
        Ok(meta) => meta,
        Err(err) => {
            eprintln!("statx() failed: {err:#}");
            std::process::exit(-1);
        }
    };

    match meta.is_mount_root() {
        None => {
            eprintln!("undetermined");
            std::process::exit(111);
        }
        Some(true) => {
            if verbose {
                println!("path is a mount point");
            }
        }
        Some(false) => {
            if verbose {
                println!("path is NOT a mount point");
            }
            std::process::exit(1);
        }
    }
}
