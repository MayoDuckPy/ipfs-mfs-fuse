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

void* mfsf_init(struct fuse_conn_info *conn, struct fuse_config *cfg) {
    // TODO: Return a struct containing user-defined options.
    mfsf_update_pin_init();
    return NULL;
}

void mfsf_destroy(void *private_data) {
    mfsf_update_pin_destroy();
}

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
        stat->st_mode = S_IFDIR | 0755;
        stat->st_nlink = mfs_stat->children + 2;
    } else if (mfs_stat->type == MFS_FILE) {
        stat->st_mode = S_IFREG | 0644;
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

int mfsf_read(const char* path, char* buf, size_t size, off_t offset, struct fuse_file_info* fi) {
    return 0;
}

int mfsf_write(const char* path, const char* buf, size_t size, off_t offset, struct fuse_file_info* fi) {
    const char* cid_ver = "1";
    char size_str[sizeof size + 1];
    char offset_str[sizeof offset + 1];

    // NOTE: IPFS handles size and offset for us.
    sprintf(size_str, "%lu", size);
    sprintf(offset_str, "%ld", offset);
    union mfsf_result result = mfsf_cmd_run(
            "w", "files write --cid-ver %s -e -t -o %s -n %s \"%s\"", 4,
            cid_ver, offset_str, size_str, path);

    if (!result.stream)
        return -errno;

    int fd = fileno(result.stream);
    int bytes_written = write(fd, buf, size);
    if (pclose(result.stream) || mfsf_update_pin() || mfsf_publish_path("/"))
        return -errno;

    return bytes_written;
}

int mfsf_readlink(const char* path, char* buf, size_t size) {
    strncpy(buf, path, size);
    buf[strlen(path) + 1] = '\0';
    return 0;
}

int mfsf_readdir(const char* path, void* buf, fuse_fill_dir_t filler,
        off_t offset, struct fuse_file_info* fi, enum fuse_readdir_flags flags) {

    union mfsf_result result = mfsf_cmd_run("r", "files ls \"%s\"", 1, path);
    if (!result.stream)
        return -errno;

    char cmd_out[BUF_SIZE];
    while (fgets(cmd_out, BUF_SIZE, result.stream)) {
        cmd_out[strcspn(cmd_out, "\n")] = '\0';
        if (filler(buf, cmd_out, NULL, offset, FUSE_FILL_DIR_PLUS))
            break;
    }

    if (pclose(result.stream))
        return -errno;

    return 0;
}

int mfsf_unlink(const char* path) {
    return mfsf_cmd_files_rm(path, false);
}

int mfsf_rmdir(const char* path) {
    return mfsf_cmd_files_rm(path, true);
}
