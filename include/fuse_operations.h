#ifndef MFSF_OPERATIONS_H
#define MFSF_OPERATIONS_H
#define FUSE_USE_VERSION 32
#include <fuse3/fuse.h>

int mfsf_getattr(const char* path, struct stat* stat, struct fuse_file_info* fi);
int mfsf_mkdir(const char* path, mode_t mode);
int mfsf_symlink(const char* from, const char* to);
int mfsf_readlink(const char* path, char* buf, size_t size);
int mfsf_readdir(const char *path, void *buf, fuse_fill_dir_t filler,
        off_t offset, struct fuse_file_info *fi, enum fuse_readdir_flags flags);
int mfsf_rmdir(const char* path);
int mfsf_unlink(const char* path);
#endif
