use std::io;

use anyhow::{Context as _, Error, bail};

use lisy::mount::ListMounts;
use lisy::mount::MountId;
use lisy::mount::StatMount;

fn usage(mut out: impl io::Write, status: i32) -> ! {
    let _ = writeln!(
        out,
        "\
usage: listmount [options...]
options:
  -v, --verbose         verbose output
"
    );

    std::process::exit(status);
}

fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);

    let mut verbose = false;

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
        };
    }

    /*

    let mount_id = MountId::from_raw(id.parse().context("invalid mount id")?);

    let stat = StatMount::builder()
        .basic_superblock_info(true)
        .mount_id(mount_id)
        .stat()
        .context("statmount failed")?;

    if verbose {
        println!("{stat:#?}");
    } else {
        bail!("TODO");
    }
    */

    let fd = lisy::pidfd::PidFd::this(Default::default())
        .context("failed to get pid fd for this process")?;

    eprintln!("{:#?}", fd.info(lisy::pidfd::GetInfoFlags::all())?);

    let mnt_ns = fd
        .mount_namespace()
        .context("failed to get current mount namespace")?;
    let info = mnt_ns.mount_info().context("failed to get mount info")?;

    if verbose {
        println!(
            "Listing {count} mounts in namespace {id:?}",
            count = info.nr_mounts,
            id = info.mnt_ns_id
        );
    }

    for id in ListMounts::here() {
        let id = id.context("listmount failed")?;
        println!("\x1b[48;5;238mid: {id:?}\x1b[0K\x1b[0m");

        let stat = StatMount::stat(id).context("statmount failed")?;
        if let Some(opts) = stat.mount_options() {
            println!("        options: {opts:?}");
        }
        if let Some(data) = stat.device() {
            println!("         device: {data:?}");
        }
        if let Some(data) = stat.superblock_magic() {
            println!("       sb magic: {data:?}");
        }
        if let Some(data) = stat.superblock_flags() {
            println!("       sb flags: {data:?}");
        }
        if let Some(data) = stat.id() {
            println!("             id: {data:?}");
        }
        if let Some(data) = stat.parent_id() {
            println!("      parent id: {data:?}");
        }
        if let Some(data) = stat.old_id() {
            println!("         old id: {data:?}");
        }
        if let Some(data) = stat.old_parent_id() {
            println!("  old parent id: {data:?}");
        }
        if let Some(data) = stat.attr() {
            println!("           attr: {data:?}");
        }
        if let Some(data) = stat.propagation() {
            println!("    propagation: {data:?}");
        }
        if let Some(data) = stat.peer_group_id() {
            println!("           peer: {data:?}");
        }
        if let Some(data) = stat.master_group_id() {
            println!("         master: {data:?}");
        }
        if let Some(data) = stat.source() {
            println!("         source: {data:?}");
        }
        if let Some(data) = stat.propagate_from() {
            println!(" propagate from: {data:?}");
        }
        if let Some(data) = stat.mount_root() {
            println!("     mount root: {data:?}");
        }
        if let Some(data) = stat.mount_point() {
            println!("    mount point: {data:?}");
        }
        if let Some(data) = stat.fs_type() {
            println!("        fs type: {data:?}");
        }
        if let Some(data) = stat.mount_namespace_id() {
            println!("    mount ns id: {data:?}");
        }
        if let Some(data) = stat.fs_subtype() {
            println!("     fs subtype: {data:?}");
        }
        if let Some(data) = stat.options() {
            for data in data {
                println!("         option: {data:?}");
            }
        }
        if let Some(data) = stat.security_options() {
            for data in data {
                println!("security option: {data:?}");
            }
        }
    }

    Ok(())
}
