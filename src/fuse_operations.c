#include <ctype.h>
#include <errno.h>
#include <time.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "fuse_operations.h"
#include "ipfs_operations.h"

#define BUF_SIZE 1024

int mfsf_getattr(const char* path, struct stat* stat, struct fuse_file_info* fi) {
    struct mfsf_stat* mfs_stat = mfsf_cmd_files_stat(path);
    if (!mfs_stat)
        return -errno;

    struct timespec current_time = { .tv_sec = time(NULL) };
    stat->st_atim  = current_time;
    stat->st_mtim  = stat->st_atim;
    stat->st_ctim  = stat->st_atim;
    stat->st_uid   = getuid();
    stat->st_gid   = getgid();

    if (mfs_stat->type == MFS_DIRECTORY) {
        stat->st_mode = S_IFDIR | 0444;
        stat->st_nlink = mfs_stat->children + 2;
    } else if (mfs_stat->type == MFS_FILE) {
        stat->st_mode = S_IFREG | 0444;
        stat->st_nlink = 1;
        stat->st_size  = mfs_stat->size;
        //stat->st_blocks = mfs_stat->children;
    }

    free(mfs_stat);
    return 0;
}

int mfsf_mkdir(const char* path, mode_t mode) {
    return mfsf_cmd_files_mkdir(path);
}

int mfsf_symlink(const char* from, const char* to) {
    return mfsf_cmd_files_cp(from, to);
}

int mfsf_readlink(const char* path, char* buf, size_t size) {
    strncpy(buf, path, size);
    buf[strlen(path) + 1] = '\0';
    return 0;
}

int mfsf_readdir(const char* path, void* buf, fuse_fill_dir_t filler,
        off_t offset, struct fuse_file_info* fi, enum fuse_readdir_flags flags) {

    FILE* proc = mfsf_cmd_run("files ls", 1, path);
    if (!proc)
        return -errno;

    char cmd_out[BUF_SIZE];
    while (fgets(cmd_out, BUF_SIZE, proc)) {
        cmd_out[strcspn(cmd_out, "\n")] = '\0';
        if (filler(buf, cmd_out, NULL, offset, FUSE_FILL_DIR_PLUS))
            break;
    }

    if (pclose(proc))
        return -errno;

    return 0;
}

int mfsf_unlink(const char* path) {
    return mfsf_cmd_files_rm(path, false);
}

int mfsf_rmdir(const char* path) {
    return mfsf_cmd_files_rm(path, true);
}
