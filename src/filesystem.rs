use fuser::{
    FileAttr, FileType, Filesystem, KernelConfig, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyWrite, Request, TimeOrNow,
};
// use ipfs_api_backend_hyper::IpfsClient;
use libc::{EEXIST, EINVAL, EIO, ENOENT, ENOSYS};
use log::{debug, error, warn};
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::os::raw::c_int;
use std::time::{Duration, SystemTime};

use crate::ipfs::IpfsFuseAdapter;
use FsError::*;

const FUSE_CAP_WRITEBACK_CACHE: u32 = 1 << 16;
const FUSE_CAP_NO_OPEN_SUPPORT: u32 = 1 << 17;
const FUSE_CAP_NO_OPENDIR_SUPPORT: u32 = 1 << 24;

// Short TTL required as actual data is handled by IPFS and not the kernel.
// A long TTL will delay content from updating.
const TTL: Duration = Duration::new(0, 50); // 0 second

#[derive(Debug)]
enum FsError {
    NoParent,
    FileExists,
    InodeNotFound,
    InvalidString,
    InvalidFiletype,
}

struct IpfsInode {
    name: String,
    parent: Option<u64>,
    children: BTreeMap<String, u64>,
}

pub struct IpfsMFS {
    pub ipfs_uri: String,
    // ipfs_client: Option<IpfsClient>,
    inodes: Box<HashMap<u64, IpfsInode>>,
}

// TODO: Print errors as they occur instead of returning them (otherwise create error enums)
impl IpfsMFS {
    pub fn new() -> IpfsMFS {
        // TODO: Initialize all directories
        IpfsMFS {
            ipfs_uri: String::from("http://127.0.0.1:5001"),
            // NOTE: Keep or create during operations because mutli-threading?
            // ipfs_client: None,
            inodes: Box::new(HashMap::from([(
                1, // Root inode = 1
                IpfsInode {
                    parent: None,
                    name: String::from(""),
                    children: BTreeMap::new(),
                },
            )])),
        }
    }

    /// Returns a reference to the associated inode data struct.
    fn get_inode(&self, inode: &u64) -> Option<&IpfsInode> {
        self.inodes.get(inode)
    }

    /// Returns a mutable reference to the associated inode data struct.
    fn get_mut_inode(&mut self, inode: &u64) -> Option<&mut IpfsInode> {
        self.inodes.get_mut(inode)
    }

    /// Writes a path to a new inode and return the inode in a Result.
    fn write_inode(&mut self, parent: u64, name: &str) -> Result<u64, FsError> {
        let ino = (self.inodes.len() + 1) as u64;
        let parent_node = self.get_mut_inode(&parent).ok_or(NoParent)?;

        // Add child to parent
        match parent_node.children.contains_key(name) {
            true => return Err(FileExists),
            false => parent_node.children.insert(name.to_string(), ino),
        };

        self.inodes.insert(
            ino,
            IpfsInode {
                parent: Some(parent),
                name: name.to_string(),
                children: BTreeMap::new(), // Unknown until traversed
            },
        );

        Ok(ino)
    }

    /// Remove an inode
    fn remove_inode(&mut self, inode: &u64) -> Result<(), FsError> {
        let mut node = self.inodes.remove(&inode).ok_or(InodeNotFound)?;

        // Remove from parent dir
        match node.parent {
            None => return Err(NoParent),
            Some(parent) => {
                let parent = self.get_mut_inode(&parent).ok_or(NoParent)?;
                parent.children.remove(&node.name);
            }
        }

        // Orphan node's children
        // TODO: Recursively remove inodes
        if node.children.is_empty() != true {
            for (_, ino) in node.children.iter_mut() {
                match self.get_mut_inode(&ino) {
                    None => return Err(InodeNotFound),
                    Some(child) => child.parent = None,
                }
            }
        };

        Ok(())
    }

    /// Recursively constructs and returns an inode's full path.
    fn get_inode_path(&self, inode: &u64) -> Result<String, FsError> {
        let node = match self.get_inode(inode) {
            None => return Err(InodeNotFound),
            Some(node) => node,
        };

        if node.parent == None {
            if node.name.is_empty() {
                // Root dir
                return Ok(String::from("/"));
            }

            return Err(NoParent);
        }

        // Traverse parent nodes to construct path
        let parent_path = match self.get_inode_path(&node.parent.ok_or(NoParent)?) {
            Ok(path) => {
                // This check prevents a '//' being prefixed for the root dir.
                // E.g. '//path/to/file' instead of '/path/to/file'.
                //
                // Things should work without this check but keep for
                // correctness anyway to prevent future bugs.
                if path.len() > 1 {
                    path
                } else {
                    String::from("")
                }
            }
            Err(e) => return Err(e),
        };

        Ok(format!("{}/{}", parent_path, node.name))
    }
}

// TODO: readlink?, symlink
impl Filesystem for IpfsMFS {
    fn init(&mut self, _req: &Request<'_>, config: &mut KernelConfig) -> Result<(), c_int> {
        match config.add_capabilities(
            FUSE_CAP_WRITEBACK_CACHE | FUSE_CAP_NO_OPEN_SUPPORT | FUSE_CAP_NO_OPENDIR_SUPPORT,
        ) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("{}", e);
                return Err(-1);
            }
        }
    }

    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let ipfs_adapter = IpfsFuseAdapter;
        let parent_path = match self.get_inode_path(&parent) {
            Ok(path) => path,
            Err(e) => {
                debug!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let name = match name.to_str() {
            Some(name) => name,
            None => {
                warn!("{:?}: {}", InvalidString, name.to_string_lossy());
                reply.error(EINVAL);
                return;
            }
        };

        let stats =
            match ipfs_adapter.mfs_stat(&self.ipfs_uri, &format!("{}/{}", parent_path, name)) {
                Ok(stats) => stats,
                Err(_) => {
                    reply.error(ENOENT);
                    return;
                }
            };

        let blocks;
        let nlink;
        let perm;
        match stats.kind {
            FileType::RegularFile => {
                blocks = stats.blocks;
                nlink = 1;
                perm = 0o644;
            }
            FileType::Directory => {
                blocks = 0;
                nlink = 2 + (stats.blocks as u32);
                perm = 0o755 | 4;
            }
            _ => {
                warn!("{:?}", InvalidFiletype);
                reply.error(ENOENT);
                return;
            }
        };

        // Get inode from parent
        let ino = match self
            .get_inode(&parent)
            .and_then(|parent| parent.children.get(name))
        {
            None => {
                reply.error(ENOENT);
                return;
            }
            Some(ino) => ino.clone(),
        };

        let attr = FileAttr {
            ino,
            size: stats.size,
            blocks,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind: stats.kind,
            perm,
            nlink,
            uid: req.uid(),
            gid: req.gid(),
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        reply.entry(&TTL, &attr, 0);
    }

    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        if !self.inodes.contains_key(&ino) {
            reply.error(ENOENT);
            return;
        }

        let path = match self.get_inode_path(&ino) {
            Ok(path) => path,
            Err(e) => {
                warn!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let ipfs_adapter = IpfsFuseAdapter;
        let stats = match ipfs_adapter.mfs_stat(&self.ipfs_uri, &path) {
            Ok(stats) => stats,
            Err(_e) => {
                reply.error(ENOENT);
                return;
            }
        };

        let blocks;
        let nlink;
        let perm;
        match stats.kind {
            FileType::RegularFile => {
                blocks = stats.blocks;
                nlink = 1;
                perm = 0o644;
            }
            FileType::Directory => {
                blocks = 0;
                nlink = 2 + (stats.blocks as u32);
                perm = 0o755 | 4;
            }
            _ => {
                warn!("{:?}", InvalidFiletype);
                reply.error(ENOSYS);
                return;
            }
        };

        let attr = FileAttr {
            ino,
            size: stats.size,
            blocks,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind: stats.kind,
            perm,
            nlink,
            uid: req.uid(),
            gid: req.gid(),
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        reply.attr(&TTL, &attr);
    }

    fn mkdir(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let ipfs_adapter = IpfsFuseAdapter;
        let parent_path = match self.get_inode_path(&parent) {
            Ok(path) => path,
            Err(e) => {
                warn!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let name = match name.to_str() {
            Some(name) => name,
            None => {
                warn!("{:?}: {}", InvalidString, name.to_string_lossy());
                reply.error(EINVAL);
                return;
            }
        };

        match ipfs_adapter.mfs_mkdir(
            &self.ipfs_uri,
            &format!("{}/{}", parent_path, name),
            false,
            1,
            true,
        ) {
            Ok(_) => {}
            Err(_) => {
                reply.error(EIO);
                return;
            }
        }

        // Allocate new inode
        let ino = match self.write_inode(parent, name) {
            Ok(ino) => ino,
            Err(e) => {
                if let FileExists = e {
                    reply.error(EEXIST);
                } else {
                    warn!("{:?}", e);
                    reply.error(EIO);
                }

                return;
            }
        };

        let attr = FileAttr {
            ino,
            size: 0,
            blocks: 0,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: req.uid(),
            gid: req.gid(),
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        reply.entry(&TTL, &attr, 0);
    }

    fn mknod(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        _rdev: u32,
        reply: ReplyEntry,
    ) {
        let path = match self.get_inode_path(&parent) {
            Ok(path) => path,
            Err(e) => {
                warn!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let name = match name.to_str() {
            Some(name) => name,
            None => {
                warn!("{:?}: {}", InvalidString, name.to_string_lossy());
                reply.error(EINVAL);
                return;
            }
        };

        // Add file to IPFS by writing a zero-size buffer
        let ipfs_adapter = IpfsFuseAdapter;
        if let Err(_) =
            ipfs_adapter.mfs_write(&self.ipfs_uri, &format!("{}/{}", path, name), 0, 0, &[])
        {
            reply.error(EIO);
            return;
        }

        // Allocate inode
        let ino = match self.write_inode(parent, name) {
            Ok(ino) => ino,
            Err(e) => {
                if let FileExists = e {
                    reply.error(EEXIST);
                } else {
                    warn!("{:?}", e);
                    reply.error(EIO);
                }

                return;
            }
        };

        // Assume we're always creating an empty file
        let attr = FileAttr {
            ino,
            size: 0,
            blocks: 0,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind: FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid: req.uid(),
            gid: req.gid(),
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        reply.entry(&TTL, &attr, ino)
    }

    // fn opendir(&mut self, _req: &Request<'_>, ino: u64, _flags: i32, reply: ReplyOpen) {
    //     // reply.error(ENOSYS);
    //     if !self.contains_key(&ino) {
    //         reply.error(ENOENT);
    //         return;
    //     }

    //     let flags: u32 = (libc::R_OK | libc::W_OK).try_into().unwrap();
    //     reply.opened(0, flags);
    // }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        let path = match self.get_inode_path(&ino) {
            Ok(path) => path,
            Err(e) => {
                warn!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let ipfs_adapter = IpfsFuseAdapter;
        let data = match ipfs_adapter.mfs_read(&self.ipfs_uri, &path, offset, size as i64) {
            Ok(data) => data,
            Err(_) => {
                reply.error(EIO);
                return;
            }
        };

        reply.data(&data);
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if !self.inodes.contains_key(&ino) {
            error!("{:?}", InodeNotFound);
            reply.error(ENOENT);
            return;
        }

        let path = match self.get_inode_path(&ino) {
            Ok(path) => path,
            Err(e) => {
                warn!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let ipfs_adapter = IpfsFuseAdapter;
        let entries = match ipfs_adapter.mfs_ls(&self.ipfs_uri, &path) {
            Ok(entries) => entries,
            Err(_) => {
                reply.error(ENOENT);
                return;
            }
        };

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // Add paths to inode table as we discover them
            let inode = match self.write_inode(ino, &entry.name) {
                Ok(inode) => inode,
                Err(_) => ino,
            };
            if reply.add(inode, (i + 1) as i64, entry.kind, entry.name) {
                break;
            }
        }

        reply.ok();
    }

    fn readlink(&mut self, _req: &Request, ino: u64, reply: ReplyData) {
        if !self.inodes.contains_key(&ino) {
            reply.error(ENOENT);
            return;
        }

        let path = match self.get_inode_path(&ino) {
            Err(_) => {
                reply.error(ENOENT);
                return;
            }
            Ok(path) => path,
        };

        reply.data(path.as_bytes());
    }

    fn rename(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        let name = name.to_str().unwrap();
        let newname = newname.to_str().unwrap();

        let parent_path = match self.get_inode_path(&parent) {
            Ok(path) => path,
            Err(e) => {
                warn!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let new_parent_path = match self.get_inode_path(&newparent) {
            Ok(path) => path,
            Err(e) => {
                warn!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let path = format!("{}/{}", &parent_path, name);
        let dest = format!("{}/{}", &new_parent_path, newname);

        // Attempt rename on IPFS
        let ipfs_adapter = IpfsFuseAdapter;
        if let Err(_) = ipfs_adapter.mfs_rename(&self.ipfs_uri, &path, &dest) {
            reply.error(ENOENT);
            return;
        };

        // Check if file already exists
        match self.get_inode(&newparent) {
            None => {
                reply.error(ENOENT);
                return;
            }
            Some(new_parent) => {
                if new_parent.children.contains_key(newname) {
                    // Cannot rename as file already exists
                    reply.error(EEXIST);
                    return;
                }
            }
        }

        let ino = match self
            .get_mut_inode(&parent)
            .and_then(|parent| parent.children.remove(name))
        {
            None => {
                reply.error(ENOENT);
                return;
            }
            Some(ino) => ino,
        };

        match self.get_mut_inode(&newparent) {
            None => {
                reply.error(ENOENT);
                return;
            }
            Some(new_parent) => new_parent.children.insert(newname.to_string(), ino),
        };

        match self.get_mut_inode(&ino) {
            None => {
                reply.error(ENOENT);
                return;
            }
            Some(ino) => {
                ino.parent = Some(newparent);
                ino.name = newname.to_string();
            }
        };

        reply.ok();
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let ipfs_adapter = IpfsFuseAdapter;
        let parent_path = match self.get_inode_path(&parent) {
            Ok(path) => path,
            Err(e) => {
                warn!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let name = match name.to_str() {
            Some(name) => name,
            None => {
                warn!("{:?}: {}", InvalidString, name.to_string_lossy());
                reply.error(EINVAL);
                return;
            }
        };

        if let Err(_) = ipfs_adapter.mfs_rm(
            &self.ipfs_uri,
            &format!("{}/{}", parent_path, name),
            true,
            false,
        ) {
            reply.error(ENOENT);
            return;
        }

        // Fetch inode so it can be deallocated
        let ino = match self
            .get_inode(&parent)
            .and_then(|parent| parent.children.get(name))
        {
            None => {
                reply.error(ENOENT);
                return;
            }
            Some(ino) => ino.clone(),
        };

        // Deallocate inode
        match self.remove_inode(&ino) {
            Ok(_) => {}
            Err(e) => {
                error!("{:?}", e);
                reply.error(EIO);
                return;
            }
        };

        reply.ok();
    }

    fn setattr(
        &mut self,
        req: &Request<'_>,
        ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let size = size.unwrap_or(0);
        let attr = FileAttr {
            ino,
            size,
            blocks: size / 512,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind: FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid: req.uid(),
            gid: req.gid(),
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        reply.attr(&TTL, &attr);
    }

    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let ipfs_adapter = IpfsFuseAdapter;
        let parent_path = match self.get_inode_path(&parent) {
            Ok(path) => path,
            Err(e) => {
                warn!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let name = match name.to_str() {
            Some(name) => name,
            None => {
                warn!("{:?}: {}", InvalidString, name.to_string_lossy());
                reply.error(EINVAL);
                return;
            }
        };

        match ipfs_adapter.mfs_rm(
            &self.ipfs_uri,
            &format!("{}/{}", parent_path, name),
            false,
            false,
        ) {
            Ok(_) => {}
            Err(_) => {
                reply.error(ENOENT);
                return;
            }
        }

        // Fetch inode so it can be deallocated
        let ino = match self
            .get_inode(&parent)
            .and_then(|parent| parent.children.get(name))
        {
            None => {
                reply.error(ENOENT);
                return;
            }
            Some(ino) => ino.clone(),
        };

        // Deallocate inode
        match self.remove_inode(&ino) {
            Ok(_) => {}
            Err(e) => {
                error!("{:?}", e);
                reply.error(EIO);
                return;
            }
        };

        reply.ok();
    }

    // FIXME: Inode corruption seemingly occurs after write
    fn write(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        let path = match self.get_inode_path(&ino) {
            Ok(path) => path,
            Err(e) => {
                warn!("{:?}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let ipfs_adapter = IpfsFuseAdapter;
        if let Err(_) =
            ipfs_adapter.mfs_write(&self.ipfs_uri, &path, offset, data.len() as i64, data)
        {
            reply.error(EIO);
            return;
        }

        reply.written(data.len() as u32);
    }
}
