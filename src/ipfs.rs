use fuser::FileType;
use futures::TryStreamExt;
use ipfs_api_backend_hyper::request::{FilesMkdir, FilesMv, FilesRead, FilesRm, FilesWrite};
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
// use log::debug;
use std::io::Cursor;

#[derive(Clone)]
pub struct IpfsFileAttr {
    // TODO: Not all values are returned by any one command so set fields to optional.
    pub name: String,
    pub blocks: u64,
    pub size: u64,
    pub kind: FileType,
    pub hash: String,
}

pub struct IpfsFuseAdapter;

// TODO: Replace io_uring instances with global runtime
// TODO: Return error if client fails to init
impl IpfsFuseAdapter {
    pub fn mfs_ls(self, uri: &str, path: &str) -> Vec<IpfsFileAttr> {
        let mut entries = vec![];

        tokio_uring::start(async {
            let client = IpfsClient::from_str(uri).unwrap();
            let files = client.files_ls(Some(path)).await.unwrap();

            for (i, entry) in files.entries.iter().enumerate() {
                let attrs = IpfsFileAttr {
                    blocks: 0, // Not given
                    size: entry.size,
                    name: entry.name.clone(),
                    hash: entry.hash.clone(),
                    kind: if entry.typ == 0 {
                        FileType::RegularFile
                    } else {
                        FileType::Directory
                    },
                };
                entries.insert(i, attrs);
            }

            entries
        })
    }

    pub fn mfs_mkdir(
        self,
        uri: &str,
        path: &str,
        parents: bool,
        cid_version: i32,
        flush: bool,
    ) -> Result<(), String> {
        let req = FilesMkdir {
            path,
            parents: Some(parents),
            hash: None,
            cid_version: Some(cid_version),
            flush: Some(flush),
        };

        tokio_uring::start(async {
            let client = IpfsClient::from_str(uri).unwrap();
            match client.files_mkdir_with_options(req).await {
                Ok(_) => return Ok(()),
                Err(e) => return Err(e.to_string()),
            };
        })
    }

    pub fn mfs_read(
        self,
        uri: &str,
        path: &str,
        offset: i64,
        count: i64,
    ) -> Result<Vec<u8>, String> {
        let req = FilesRead {
            path,
            offset: Some(offset),
            count: Some(count),
        };

        tokio_uring::start(async {
            let client = IpfsClient::from_str(uri).unwrap();
            let res = match client
                .files_read_with_options(req)
                .map_ok(|chunk| chunk.to_vec())
                .try_concat()
                .await
            {
                Ok(res) => res,
                Err(e) => return Err(e.to_string()),
            };

            Ok(res.to_vec())
        })
    }

    pub fn mfs_rename(self, uri: &str, path: &str, dest: &str) -> Result<(), String> {
        let req = FilesMv {
            path,
            dest,
            flush: Some(true),
        };

        tokio_uring::start(async {
            let client = IpfsClient::from_str(uri).unwrap();
            match client.files_mv_with_options(req).await {
                Ok(_) => return Ok(()),
                Err(e) => return Err(e.to_string()),
            };
        })
    }

    pub fn mfs_rm(self, uri: &str, path: &str, recursive: bool, force: bool) -> Result<(), String> {
        let req = FilesRm {
            path,
            recursive: Some(recursive),
            flush: Some(force),
        };

        tokio_uring::start(async {
            let client = IpfsClient::from_str(uri).unwrap();
            match client.files_rm_with_options(req).await {
                Ok(_) => return Ok(()),
                Err(e) => return Err(e.to_string()),
            };
        })
    }

    pub fn mfs_stat(self, uri: &str, path: &str) -> Result<IpfsFileAttr, String> {
        tokio_uring::start(async {
            let client = IpfsClient::from_str(uri).unwrap();
            let stats = match client.files_stat(&path).await {
                Ok(stats) => stats,
                Err(e) => return Err(e.to_string()),
            };

            let attr = IpfsFileAttr {
                size: stats.size,
                blocks: stats.blocks,
                name: stats.hash.clone(),
                hash: stats.hash.clone(),
                kind: if stats.typ == "file" {
                    FileType::RegularFile
                } else {
                    FileType::Directory
                },
            };

            Ok(attr)
        })
    }

    pub fn mfs_write(
        self,
        uri: &str,
        path: &str,
        offset: i64,
        count: i64,
        data: &[u8],
    ) -> Result<(), String> {
        let req = FilesWrite {
            path,
            offset: Some(offset),
            count: Some(count),
            create: Some(true),
            truncate: Some(true),
            cid_version: Some(1),
            flush: Some(true),
            parents: None,
            raw_leaves: None,
            hash: None,
        };

        // NOTE: Data is copied to create Cursor.
        tokio_uring::start(async {
            let client = IpfsClient::from_str(uri).unwrap();
            let cursor = Cursor::new(data.to_owned());
            match client.files_write_with_options(req, cursor).await {
                Err(e) => Err(e.to_string()),
                _ => Ok(()),
            }
        })
    }
}
