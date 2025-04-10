use std::io;
use std::os::unix::ffi::OsStrExt;

use anyhow::{Context as _, Error};

use lisy::mount::{MountAttr, MoveMount, OpenTree};

fn usage(mut out: impl io::Write, status: i32) -> ! {
    let _ = writeln!(
        out,
        "\
usage: bindmount [options...] source subdir destination
options:
  -r                    do a recursive mount
"
    );

    std::process::exit(status);
}

fn main() -> Result<(), Error> {
    let mut args = std::env::args_os().skip(1);

    let mut recursive = false;
    let mut source = None;

    for arg_os in args.by_ref() {
        let arg = arg_os.as_bytes();

        if arg == b"-h" || arg == b"--help" {
            usage(std::io::stdout(), 0);
        }

        if arg == b"-r" || arg == b"--recursive" {
            recursive = true;
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

    let subdir = match args.next() {
        Some(p) => p,
        None => usage(std::io::stderr(), 1),
    };

    let dest = match args.next() {
        Some(p) => p,
        None => usage(std::io::stderr(), 1),
    };

    let mut otflags = OpenTree::CLOEXEC | OpenTree::CLONE;
    if recursive {
        otflags |= OpenTree::RECURSIVE
    };

    let fs = lisy::mount::Fs::open("xfs", lisy::mount::FsOpen::CLOEXEC)
        .context("fsopen failed for 'xfs' file system")?;
    fs.set_string("source", &source)
        .context("failed to configure 'source' for file system handle")?;
    let main_mount = fs
        .create()
        .context("failed to open superblock")?
        .mount(
            lisy::mount::FsMount::CLOEXEC,
            MountAttr::NOSUID
                | MountAttr::NODEV
                | MountAttr::NOEXEC
                | MountAttr::RELATIME
                | MountAttr::NOSYMFOLLOW,
        )
        .context("failed to open mount")?;

    let submount = unsafe {
        main_mount
            .open_subtree(&subdir)
            .context("failed to open subtree")?
    };

    submount
        .move_mount(&dest, MoveMount::empty())
        .context("failed to move the mount into place")?;

    Ok(())
}
