use std::io;

use lisy::fs::ReadDir;

fn main() -> io::Result<()> {
    let mut done = false;
    for arg in std::env::args_os().skip(1) {
        done = true;
        for entry in ReadDir::read(&arg)? {
            let entry = entry?;
            println!("{entry:?}");
        }
    }
    if !done {
        for entry in ReadDir::read(".")? {
            let entry = entry?;
            println!("{entry:?}");
        }
    }
    Ok(())
}
