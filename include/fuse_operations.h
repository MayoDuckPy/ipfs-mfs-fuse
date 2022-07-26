#ifndef MFSF_OPERATIONS_H
#define MFSF_OPERATIONS_H
#define FUSE_USE_VERSION 32
#include <fuse3/fuse.h>

enum mfsf_ipfs_addr_type {
    INVALID_ADDR,
    IPFS_ADDR,
    IPNS_ADDR
};

enum mfs_type {
    MFS_DIRECTORY,
    MFS_FILE
};

struct mfsf_path {
    char* parent_dir;  // The relative dir in our filesystem
    char* ipfs_addr;   // e.g. "/ipfs/<CID>"
    char* mfs_name;    // The node name in the MFS
    enum mfsf_ipfs_addr_type addr_type;  // IPFS address type
};

struct mfsf_stat {
    //char* name;
    int size;
    int cumulative_size;
    int children;
    enum mfs_type type;
};

struct mfsf_path* mfsf_path_create(const char* path);
void mfsf_path_destroy(struct mfsf_path* mfsf_path);

int mfsf_getattr(const char* path, struct stat* stat, struct fuse_file_info* fi);
int mfsf_mkdir(const char* path, mode_t mode);
int mfsf_symlink(const char* from, const char* to);
int mfsf_readlink(const char* path, char* buf, size_t size);
int mfsf_readdir(const char *path, void *buf, fuse_fill_dir_t filler,
        off_t offset, struct fuse_file_info *fi, enum fuse_readdir_flags flags);
#endif
