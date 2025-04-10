# **Li**nux specific **Sy**stem API

Higher level APIs targeting newer Linux kernel features.

This crate provides somewhat higher level access to more modern features of the Linux kernel, such
as builder-style access to the new mount API, `openat2(2)` call with a builder for the `struct
open_how` parameters, all the new features of the `statx(2)` system call (such as finding out
whether a path is a mount point), or to build user namespace file descriptors (which requires
spawning processes and is therefore somewhat inconvenient to do manually).

See the documentation for details.

```
$ cargo doc --no-deps --open
```

# Contribution

See the `CONTRIBUTING.md` file.
