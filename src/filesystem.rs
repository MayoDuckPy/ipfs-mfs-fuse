use fuser::{
    FileAttr, FileType, Filesystem, KernelConfig, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyWrite, Request, TimeOrNow,
};
// use ipfs_api_backend_hyper::IpfsClient;
use libc::{EIO, ENOENT, ENOSYS};
use log::{error, info};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::raw::c_int;
use std::time::{Duration, SystemTime};

use crate::ipfs::IpfsFuseAdapter;

const FUSE_CAP_NO_OPEN_SUPPORT: u32 = 1 << 17;
const FUSE_CAP_NO_OPENDIR_SUPPORT: u32 = 1 << 24;

struct IpfsInode {
    name: String,
    parent: Option<u64>,
    children: Option<Vec<u64>>, // TODO: Use HashMap<String, u64>
}

pub struct IpfsMFS {
    pub ipfs_uri: String,
    // ipfs_client: Option<IpfsClient>,
    inodes: Box<HashMap<u64, IpfsInode>>,
}

// Short TTL required as actual data is handled by IPFS and not the kernel.
// A long TTL will delay content from updating.
const TTL_0: Duration = Duration::new(0, 50); // 0 second

// TODO: Print errors as they occur instead of returning them (otherwise create error enums)
impl IpfsMFS {
    pub fn new() -> IpfsMFS {
        IpfsMFS {
            ipfs_uri: String::from("http://127.0.0.1:5001"),
            // NOTE: Keep or create during operations because mutli-threading?
            // ipfs_client: None,
            inodes: Box::new(HashMap::from([(
                1, // Root inode = 1
                IpfsInode {
                    parent: None,
                    name: String::from(""),
                    children: None,
                },
            )])),
        }
    }

    fn contains_key(&self, inode: &u64) -> bool {
        self.inodes.contains_key(inode)
    }

    /// Returns a reference to the associated inode data struct.
    fn get_inode(&self, inode: &u64) -> Option<&IpfsInode> {
        self.inodes.get(inode)
    }

    /// Returns a mutable reference to the associated inode data struct.
    fn get_mut_inode(&mut self, inode: &u64) -> Option<&mut IpfsInode> {
        self.inodes.get_mut(inode)
    }

    /// Writes a path to a new inode and return the inode in a Result. Otherwise,
    /// returns an error string.
    fn write_inode(&mut self, parent: u64, name: &str) -> Result<u64, String> {
        // Check if parent contains child with the same name.
        // We can assume parent node exists during normal operation.
        let parent_node = self.inodes.get(&parent).unwrap();
        if parent_node.children != None {
            for child in parent_node.children.as_ref().unwrap() {
                if name == self.inodes.get(&child).unwrap().name {
                    return Err(String::from("Inode already exists"));
                }
            }
        }

        let ino = (self.inodes.len() + 1) as u64;
        {
            // Add child to parent
            let parent_node = self.get_mut_inode(&parent).expect("Parent should exist");
            match parent_node.children.as_mut() {
                None => parent_node.children = Some(Vec::from([ino])),
                Some(children) => children.push(ino),
            }
        }

        self.inodes.insert(
            ino,
            IpfsInode {
                parent: Some(parent),
                name: name.to_string(),
                children: None, // Unknown until traversed
            },
        );

        Ok(ino)
    }

    // TODO: Refactor
    fn remove_inode(&mut self, inode: &u64) -> Result<(), String> {
        let mut node = match self.inodes.remove(&inode) {
            Some(node) => node,
            None => return Err(String::from("Failed to remove inode")),
        };

        // Remove from parent dir
        match node.parent {
            None => {}
            Some(parent) => {
                let parent = self.get_mut_inode(&parent).expect("Parent should exist");
                match parent.children.as_mut() {
                    None => {}
                    Some(children) => {
                        for (i, child) in children.iter().enumerate() {
                            if inode == child {
                                children.remove(i);
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Orphan node's children
        // TODO: Recursively remove inodes
        match node.children.as_mut() {
            None => {}
            Some(children) => {
                for child in children.iter_mut() {
                    match self.get_mut_inode(&child) {
                        None => return Err(String::from("Orphaned inode does not exist")),
                        Some(child) => child.parent = None,
                    }
                }
            }
        };

        Ok(())
    }

    /// Recursively constructs and returns an inode's full path.
    fn get_inode_path(&self, inode: &u64) -> Result<String, String> {
        let node = match self.get_inode(inode) {
            None => return Err(String::from("Invalid inode")),
            Some(node) => node,
        };

        if node.parent == None {
            if node.name.is_empty() {
                // Root dir
                return Ok(String::from("/"));
            }

            return Err(String::from("Orphaned inode"));
        }

        // Traverse parent nodes to construct path
        let parent_path = match self.get_inode_path(&node.parent.unwrap()) {
            Ok(path) => {
                // This check prevents a '//' being prefixed for the root dir.
                // E.g. '//path/to/file' instead of '/path/to/file'.
                //
                // Things should work without this check but this is good for
                // posterity and debugging.
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
        match config.add_capabilities(FUSE_CAP_NO_OPEN_SUPPORT | FUSE_CAP_NO_OPENDIR_SUPPORT) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("init: {}", e);
                return Err(-1);
            }
        }
    }

    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let ipfs_adapter = IpfsFuseAdapter;
        let parent_path = match self.get_inode_path(&parent) {
            Ok(path) => path,
            Err(e) => {
                error!("lookup: Failed to get parent path ({})", e);
                reply.error(ENOENT);
                return;
            }
        };

        let name = match name.to_str() {
            Some(name) => name,
            None => {
                error!("lookup: Failed converting OsStr to str");
                reply.error(ENOENT);
                return;
            }
        };

        let stats =
            match ipfs_adapter.mfs_stat(&self.ipfs_uri, &format!("{}/{}", parent_path, name)) {
                Ok(stats) => stats,
                Err(e) => {
                    info!("lookup: {}", e);
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
                reply.error(ENOENT);
                return;
            }
        };

        // Get inode of file
        let mut ino: u64 = 1;
        let parent_node = self.get_inode(&parent).expect("Parent should exist");
        if parent_node.children == None {
            reply.error(ENOENT);
            return;
        }

        // Get inode from parent
        for child in parent_node.children.as_ref().unwrap() {
            let child_name = match self.get_inode(&child) {
                None => {
                    error!("lookup: Child did not exist in parent dir");
                    reply.error(ENOENT);
                    return;
                }
                Some(child) => &child.name,
            };

            if name == child_name {
                ino = child.clone();
                break;
            }
        }

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

        reply.entry(&TTL_0, &attr, 0);
    }

    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        if self.inodes.contains_key(&ino) != true {
            reply.error(ENOENT);
            return;
        }

        let path = match self.get_inode_path(&ino) {
            Ok(path) => path,
            Err(e) => {
                error!("getattr: Failed to get parent path ({})", e);
                reply.error(ENOENT);
                return;
            }
        };

        let ipfs_adapter = IpfsFuseAdapter;
        let stats = match ipfs_adapter.mfs_stat(&self.ipfs_uri, &path) {
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

        reply.attr(&TTL_0, &attr);
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
                error!("mkdir: Failed to get parent path ({})", e);
                reply.error(ENOENT);
                return;
            }
        };

        let name = match name.to_str() {
            Some(name) => name,
            None => {
                error!("mkdir: Failed converting OsStr to Str");
                reply.error(ENOENT);
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
            Err(e) => {
                error!("mkdir: {}", e);
                reply.error(EIO);
                return;
            }
        }

        // Allocate new inode
        let ino = match self.write_inode(parent, name) {
            Ok(ino) => ino,
            Err(e) => {
                error!("mkdir: {}", e);
                reply.error(EIO);
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

        reply.entry(&TTL_0, &attr, 0);
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
            Err(_) => {
                reply.error(ENOENT);
                return;
            }
        };

        let name = name.to_str().unwrap();

        // Add file to IPFS by writing a zero-size buffer
        let ipfs_adapter = IpfsFuseAdapter;
        if let Err(e) =
            ipfs_adapter.mfs_write(&self.ipfs_uri, &format!("{}/{}", path, name), 0, 0, &[])
        {
            error!("mknod: {}", e);
            reply.error(EIO);
            return;
        }

        // Allocate inode
        let ino = match self.write_inode(parent, name) {
            Ok(ino) => ino,
            Err(e) => {
                error!("Failed to write inode {}", e);
                reply.error(EIO);
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

        reply.entry(&TTL_0, &attr, ino)
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
            Err(_) => {
                reply.error(ENOENT);
                return;
            }
        };

        let ipfs_adapter = IpfsFuseAdapter;
        let data = match ipfs_adapter.mfs_read(&self.ipfs_uri, &path, offset, size as i64) {
            Ok(data) => data,
            Err(e) => {
                error!("read: {}", e);
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
        if self.inodes.contains_key(&ino) == false {
            reply.error(ENOENT);
            return;
        }

        let path = match self.get_inode_path(&ino) {
            Ok(path) => path,
            Err(e) => {
                error!("readdir: Failed to get parent path ({})", e);
                reply.error(ENOENT);
                return;
            }
        };

        let ipfs_adapter = IpfsFuseAdapter;
        let entries = ipfs_adapter.mfs_ls(&self.ipfs_uri, &path);

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
        if self.contains_key(&ino) == false {
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
                error!("rename: Failed to get parent path ({})", e);
                reply.error(ENOENT);
                return;
            }
        };

        let new_parent_path = match self.get_inode_path(&newparent) {
            Ok(path) => path,
            Err(e) => {
                error!("rename: Failed to get parent path ({})", e);
                reply.error(ENOENT);
                return;
            }
        };

        let path = format!("{}/{}", &parent_path, name);
        let dest = format!("{}/{}", &new_parent_path, newname);

        // Attempt rename on IPFS
        let ipfs_adapter = IpfsFuseAdapter;
        if let Err(e) = ipfs_adapter.mfs_rename(&self.ipfs_uri, &path, &dest) {
            error!("rename: {}", e);
            reply.error(ENOENT);
            return;
        };

        // Fetch inode so it can be modified
        let mut ino = None;
        {
            let parent_node = self
                .get_inode(&parent)
                .expect("rename: Parent should exist");
            for child in parent_node.children.as_deref().unwrap() {
                let child_ino = self.get_inode(&child).expect("Child should exist");
                if name == child_ino.name {
                    ino = Some(*child);
                    break;
                }
            }
        }

        let ino = match ino {
            None => {
                error!("Failed to fetch inode");
                reply.error(ENOENT);
                return;
            }
            Some(ino) => ino,
        };

        match self.inodes.get_many_mut([&parent, &newparent]) {
            None => {
                // Check if call failed due to duplicate inodes
                if parent != newparent {
                    error!("Failed to fetch parent inodes");
                    reply.error(ENOENT);
                    return;
                }
            }
            Some(inodes) => {
                let [prev_parent, new_parent] = inodes;

                // Detach inode from previous parent
                // let prev_parent = self.get_mut_inode(&parent).expect("Parent should exist");
                match prev_parent.children.as_mut() {
                    None => {
                        reply.error(ENOENT);
                        return;
                    }
                    Some(children) => {
                        children.retain(|&e| e != ino);
                    }
                }

                // Add inode to new parent
                // let new_parent = self.get_mut_inode(&newparent).expect("Parent should exist");
                match new_parent.children.as_mut() {
                    None => new_parent.children = Some(Vec::from([ino])),
                    Some(children) => children.push(ino),
                }
            }
        }

        match self.get_mut_inode(&ino) {
            None => {
                reply.error(ENOENT);
                return;
            }
            Some(ino) => {
                ino.parent = Some(newparent);
                ino.name = String::from(newname);
            }
        };

        reply.ok();
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let ipfs_adapter = IpfsFuseAdapter;
        let parent_path = match self.get_inode_path(&parent) {
            Ok(path) => path,
            Err(e) => {
                error!("rmdir: Failed to get parent path ({})", e);
                reply.error(ENOENT);
                return;
            }
        };

        let name = match name.to_str() {
            Some(name) => name,
            None => {
                error!("rmdir: Failed converting OsStr to Str");
                reply.error(ENOENT);
                return;
            }
        };

        if let Err(e) = ipfs_adapter.mfs_rm(
            &self.ipfs_uri,
            &format!("{}/{}", parent_path, name),
            true,
            false,
        ) {
            error!("rmdir: {}", e);
            reply.error(ENOENT);
            return;
        }

        // Fetch inode so it can be deallocated
        let mut ino = None;
        {
            let parent_node = self.get_inode(&parent).unwrap(); // Will exist
            for child in parent_node.children.as_ref().unwrap() {
                if name == self.get_inode(&child).unwrap().name {
                    ino = Some(child.clone());
                    break;
                }
            }
        }

        // Deallocate inode
        match ino {
            None => {
                error!("rmdir: Failed to deallocate inode");
                reply.error(EIO);
                return;
            }
            Some(ino) => match self.remove_inode(&ino) {
                Ok(_) => {}
                Err(e) => {
                    error!("rmdir: {}", e);
                    reply.error(EIO);
                    return;
                }
            },
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

        reply.attr(&TTL_0, &attr);
    }

    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let ipfs_adapter = IpfsFuseAdapter;
        let parent_path = match self.get_inode_path(&parent) {
            Ok(path) => path,
            Err(e) => {
                error!("unlink: Failed to get parent path ({})", e);
                reply.error(ENOENT);
                return;
            }
        };

        let name = match name.to_str() {
            Some(name) => name,
            None => {
                error!("unlink: Failed converting OsStr to Str");
                reply.error(ENOENT);
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
            Err(e) => {
                error!("unlink: {}", e);
                reply.error(ENOENT);
                return;
            }
        }

        // Fetch inode so it can be deallocated
        let mut ino = None;
        {
            let parent_node = self.get_inode(&parent).unwrap(); // Will exist
            for child in parent_node.children.as_ref().unwrap() {
                if name == self.get_inode(&child).unwrap().name {
                    ino = Some(child.clone());
                    break;
                }
            }
        }

        // Deallocate inode
        match ino {
            None => {
                error!("unlink: Failed to deallocate inode");
                reply.error(EIO);
                return;
            }
            Some(ino) => match self.remove_inode(&ino) {
                Ok(_) => {}
                Err(e) => {
                    error!("unlink: {}", e);
                    reply.error(EIO);
                    return;
                }
            },
        };

        reply.ok();
    }

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
                error!("write: {}", e);
                reply.error(ENOENT);
                return;
            }
        };

        let ipfs_adapter = IpfsFuseAdapter;
        if let Err(e) =
            ipfs_adapter.mfs_write(&self.ipfs_uri, &path, offset, data.len() as i64, data)
        {
            error!("write: {}", e);
            reply.error(EIO);
            return;
        }

        reply.written(data.len() as u32);
    }
}
