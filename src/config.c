#include <fuse_opt.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#include "config.h"

/* A macro for a fuse_opt entry whose value will be stored in a struct
 * mfsf_config.
 */
#define mfsf_option(t, p ,v) { t, offsetof(struct mfsf_config, p), v }

static struct mfsf_config mfsf_config = {0};

static struct fuse_opt options[] = {
    mfsf_option("ipfs-bin=%s", ipfs_bin, 0),
    FUSE_OPT_END
};

struct mfsf_config* mfsf_get_config() {
    return &mfsf_config;
}

struct fuse_opt* mfsf_get_options() {
    return options;
}

/* Set defaults for the global config if they were not specified. */
void mfsf_set_config_defaults() {
    if (!mfsf_config.ipfs_bin)
        mfsf_config.ipfs_bin = "ipfs";
}
