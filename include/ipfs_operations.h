#ifndef MFS_IPFS_OP_H
#include <stdbool.h>
#include <stdio.h>

#define MFS_IPFS_OP_H
#define IPFS_BIN "ipfsp" " "
#define CID_MAX  60

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


FILE* mfsf_cmd_run(const char* cmd, int argc, ...);
int mfsf_cmd_files_cp(const char* from, const char* to);
int mfsf_cmd_files_mkdir(const char* path);
struct mfsf_stat* mfsf_cmd_files_stat(const char* path);
int mfsf_cmd_files_rm(const char* path, bool recursive);
int mfsf_cmd_pin_add(const char* path);
int mfsf_publish_path(const char* path);
#endif
