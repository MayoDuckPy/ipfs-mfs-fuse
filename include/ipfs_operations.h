#ifndef MFS_IPFS_OP_H
#define MFS_IPFS_OP_H

#include <stdbool.h>
#include <stdio.h>

#define CID_MAX  60

union mfsf_result {
    FILE* stream;
    int   result;
};

enum mfs_type {
    MFS_DIRECTORY,
    MFS_FILE
};

struct mfsf_stat {
    //char* hash;
    int size;
    int cumulative_size;
    int children;
    enum mfs_type type;
};


union mfsf_result mfsf_cmd_run(const char* cmd, int argc, const char* pipe_type, ...);
int mfsf_cmd_files_cp(const char* from, const char* to);
int mfsf_cmd_files_mkdir(const char* path);
struct mfsf_stat* mfsf_cmd_files_stat(const char* path);
int mfsf_cmd_files_rm(const char* path, bool recursive);
int mfsf_cmd_pin_add(const char* path);
int mfsf_cmd_files_rename(const char* src, const char* dst);
int mfsf_cmd_pin_rm(const char* path);
int mfsf_publish_path(const char* path);
int mfsf_update_pin();
void mfsf_update_pin_init();
void mfsf_update_pin_destroy();
#endif
