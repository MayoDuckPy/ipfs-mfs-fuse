#define FUSE_USE_VERSION 30
#include <fuse3/fuse.h>
#include <stdio.h>

#include "fuse_operations.h"

int main(int argc, char** argv) {
    // TODO: Add option to specify IPFS command and store in FUSE context
    struct fuse_operations mfsf_operations = {
        .getattr  = mfsf_getattr,
        .mknod    = NULL,
        .mkdir    = NULL,
        .open     = NULL,
        .symlink  = mfsf_symlink,
        .read     = NULL,
        .readdir  = mfsf_readdir,
        .readlink = mfsf_readlink,
        .rename   = NULL,
        .unlink   = NULL,
        .write    = NULL,
    };

    fuse_main(argc, argv, &mfsf_operations, NULL);
}
