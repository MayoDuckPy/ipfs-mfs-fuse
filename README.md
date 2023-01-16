## NOTICE

This project will be rewritten in Rust to make use of the
[ipfs-api](https://crates.io/crates/ipfs-api) crate as it is well-maintained
and will greatly improve the performance of the current implementation.

Afterwards, the current codebase will be available in the `legacy` branch.

---

# FUSE for the IPFS Mutable Filesystem

This project aims to provide a usable FUSE implementation to allow a read-write
accessible directory to be mounted in user-space.

Currently uses commands to [Kubo](https://github.com/ipfs/kubo/), an IPFS
implementation written in Go, to handle transactions.

## Building and Running

In the project directory run

```
meson build --buildtype=release
```

Compile the project with

```
meson compile -C build
```

Now, we are ready to run the project

```
cd build
./ipfs-mfs-fuse <mountpoint>
```

See [Options](#Options) for all available flags.

## Options

Use `ipfs-mfs-fuse -h` to see general FUSE options.

<br>

**Note that flags use the syntax: `-o FLAG=VALUE`.**

Example: `./ipfs-mfs-fuse -o cid-ver=1`

| Flag            | Description                                           |
| :---            | :---                                                  |
| `cid-ver`       | CID Version to be used when adding files to the MFS.  |
| `ipfs-bin`      | Path to a Kubo binary.  (To be deprecated)            |
| `ipfs-path`     | Path to a IPFS installation.  (To be deprecated)      |
| `ipfs-api`      | (Planned) Address to a running IPFS RPC API instance. |
| `ipfs-api-port` | (Planned) Port of a running IPFS RPC API instance.    |
