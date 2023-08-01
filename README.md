# FUSE for the IPFS Mutable Filesystem

This project aims to provide a FUSE implementation for the Mutable File
System (MFS) in [Kubo](https://github.com/ipfs/kubo/) with read-write
functionality.

## Running the project

Clone this repository and, from the project's root directory, run

```
cargo run <mountpoint>
```

where `mountpoint` is the directory to mount the filesystem.

Use `cargo run -- -h` to see options.
