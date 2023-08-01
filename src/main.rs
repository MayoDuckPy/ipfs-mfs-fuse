use clap::{Arg, Command};
use fuser::MountOption;
use log::log_enabled;
use log::Level::Debug;

use ipfs_mfs_fuse::filesystem::IpfsMFS;

fn main() {
    env_logger::init();
    let matches = Command::new("ipfs-mfs-fuse")
        .arg(
            Arg::new("mountpoint")
                .required(true)
                .index(1)
                .help("Mount FUSE at the given path"),
        )
        .arg(
            Arg::new("auto-unmount")
                .long("auto-unmount")
                .help("Automatically unmount on process exit"),
        )
        .arg(
            Arg::new("allow-root")
                .long("allow-root")
                .help("Allow root user to access filesystem"),
        )
        .get_matches();

    let mountpoint = matches
        .get_one::<String>("mountpoint")
        .expect("Invalid mount point");

    let mut options = vec![
        MountOption::RW,
        MountOption::Exec,
        MountOption::NoAtime,
        MountOption::FSName("IPFS Mutable File System".to_string()),
    ];

    if matches.contains_id("auto-unmount") {
        options.push(MountOption::AutoUnmount);
    }
    if matches.contains_id("allow-root") {
        options.push(MountOption::AllowRoot);
    }

    if log_enabled!(Debug) {
        options.push(MountOption::CUSTOM(String::from("debug")));
    }

    let fs = IpfsMFS::new();
    fuser::mount2(fs, mountpoint, &options).unwrap();
}
