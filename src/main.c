#include <errno.h>
#include <stdio.h>

#include "config.h"
#include "fuse_operations.h"

int main(int argc, char** argv) {
    struct fuse_operations mfsf_operations = {
        .init     = mfsf_init,
        .destroy  = mfsf_destroy,
        .getattr  = mfsf_getattr,
        .mknod    = NULL,
        .mkdir    = mfsf_mkdir,
        .open     = NULL,
        .symlink  = mfsf_symlink,
        .read     = mfsf_read,
        .readdir  = mfsf_readdir,
        .readlink = mfsf_readlink,
        .rename   = mfsf_rename,
        .rmdir    = mfsf_rmdir,
        .unlink   = mfsf_unlink,
        .write    = mfsf_write,
    };

    struct mfsf_context context = {
        .config = mfsf_get_config(),
        .handle_symlink = false
    };

    if (!context.config)
        return -ENOMEM;

    struct fuse_args args = FUSE_ARGS_INIT(argc, argv);

    fuse_opt_parse(&args, context.config, mfsf_get_options(), NULL);
    return fuse_main(args.argc, args.argv, &mfsf_operations, &context);
}
