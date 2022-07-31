#include <stdio.h>

#include "fuse_operations.h"

int main(int argc, char** argv) {
    // TODO: Add option to specify IPFS command and store in FUSE context
    struct fuse_operations mfsf_operations = {
        .init     = mfsf_init,
        .destroy  = mfsf_destroy,
        .getattr  = mfsf_getattr,
        .mknod    = NULL,
        .mkdir    = mfsf_mkdir,
        .open     = NULL,
        .symlink  = mfsf_symlink,
        .read     = NULL,
        .readdir  = mfsf_readdir,
        .readlink = mfsf_readlink,
        .rename   = NULL,
        .rmdir    = mfsf_rmdir,
        .unlink   = mfsf_unlink,
        .write    = NULL,
    };

    fuse_main(argc, argv, &mfsf_operations, NULL);
}
