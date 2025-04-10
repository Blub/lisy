use std::io;
use std::os::unix::ffi::OsStrExt;

use anyhow::{Context as _, Error};

use lisy::mount::{Mount, MountSetAttr, MoveMount, OpenTree};
use lisy::userns::{IdMapping, Userns};

fn usage(mut out: impl io::Write, status: i32) -> ! {
    let _ = writeln!(
        out,
        "\
usage: idmapmount [options...] source destination
options:
  -r                    do a recursive mount
  -u host:ns:count      map a user id range
  -g host:ns:count      map a group id range
  -b host:ns:count      map both
"
    );

    std::process::exit(status);
}

fn main() -> Result<(), Error> {
    let mut args = std::env::args_os().skip(1);

    let mut recursive = false;
    let mut source = None;
    let mut uid_mappings = Vec::new();
    let mut gid_mappings = Vec::new();
    while let Some(arg_os) = args.next() {
        let arg = arg_os.as_bytes();

        if arg == b"-h" || arg == b"--help" {
            usage(std::io::stdout(), 0);
        }

        if arg == b"-r" || arg == b"--recursive" {
            recursive = true;
            continue;
        }

        let (kind, arg) = if let Some(arg) = arg.strip_prefix(b"-u") {
            ('u', arg)
        } else if let Some(arg) = arg.strip_prefix(b"-g") {
            ('g', arg)
        } else if let Some(arg) = arg.strip_prefix(b"-b") {
            ('b', arg)
        } else if arg == b"--" {
            break;
        } else if arg.starts_with(b"-") {
            usage(std::io::stderr(), 1);
        } else {
            source = Some(arg_os);
            break;
        };

        let mapping_owned;
        let mapping = if arg.is_empty() {
            let Some(arg) = args.next() else {
                usage(std::io::stderr(), 1)
            };
            mapping_owned = arg;
            mapping_owned.as_bytes()
        } else {
            arg
        };

        let mapping = IdMapping::parse_common(std::str::from_utf8(mapping)?)?;

        if kind == 'u' || kind == 'b' {
            uid_mappings.push(mapping);
        }
        if kind == 'g' || kind == 'b' {
            gid_mappings.push(mapping);
        }
    }

    let source = match source.or_else(|| args.next()) {
        Some(p) => p,
        None => usage(std::io::stderr(), 1),
    };

    let dest = match args.next() {
        Some(p) => p,
        None => usage(std::io::stderr(), 1),
    };

    let userns = Userns::builder().context("failed to prepare user namespace")?;
    userns
        .map_gids(&gid_mappings)
        .context("failed to map group ids")?;
    userns
        .map_uids(&uid_mappings)
        .context("failed to map user ids")?;
    let userns = userns
        .into_fd()
        .context("failed to finish creating user namespace")?;

    let mut otflags = OpenTree::CLOEXEC | OpenTree::CLONE;
    if recursive {
        otflags |= OpenTree::RECURSIVE
    };
    let mount = Mount::open_tree(&source, otflags, 0).context("open_tree failed on mount point")?;

    mount
        .setattr(
            &MountSetAttr::new().idmap(&userns),
            libc::AT_RECURSIVE | libc::AT_NO_AUTOMOUNT,
        )
        .context("failed to apply idmapping to mount tree")?;

    mount
        .move_mount(&dest, MoveMount::empty())
        .context("failed to move mount into place")?;

    Ok(())
}
