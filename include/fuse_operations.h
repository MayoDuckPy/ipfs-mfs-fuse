#ifndef MFSF_OPERATIONS_H
#define MFSF_OPERATIONS_H
#define FUSE_USE_VERSION 32
#include <fuse3/fuse.h>

void* mfsf_init(struct fuse_conn_info *conn, struct fuse_config *cfg);
void mfsf_destroy(void *private_data);
int mfsf_getattr(const char* path, struct stat* stat, struct fuse_file_info* fi);
int mfsf_mkdir(const char* path, mode_t mode);
int mfsf_symlink(const char* from, const char* to);
int mfsf_read(const char* path, char* buf, size_t size, off_t offset, struct fuse_file_info* fi);
int mfsf_write(const char* path, const char* buf, size_t size, off_t offset, struct fuse_file_info* fi);
int mfsf_readlink(const char* path, char* buf, size_t size);
int mfsf_readdir(const char *path, void *buf, fuse_fill_dir_t filler,
        off_t offset, struct fuse_file_info *fi, enum fuse_readdir_flags flags);
int mfsf_rename(const char* src, const char* dst, unsigned int flags);
int mfsf_rmdir(const char* path);
int mfsf_unlink(const char* path);
#endif
