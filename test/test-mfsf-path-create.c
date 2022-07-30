#include <string.h>
#include <stdlib.h>

#include "fuse_operations.c"
#include "ipfs_operations.c"

#define TEST(x, errno) if ((err = x)) return err

enum ERROR_STATUS {
    MALLOC_ERR = -1,
    NO_ERR,
    EMPTY_ADDR_ERR,
    IPFS_ADDR_ERR,
    IPNS_ADDR_ERR,
    DIR_WITH_IPFS_ADDR_ERR,
    NON_IPFS_ADDR_ERR,
};

static int parse_empty_addr() {
    struct mfsf_path* path = mfsf_path_create("");
    if (path)
        return EMPTY_ADDR_ERR;

    return NO_ERR;
}

static int parse_ipfs_addr() {
    struct mfsf_path* path = mfsf_path_create("/ipfs/QmetARxCz9iCcLyTdVCCpbJpJ4jxpTB5FxF4Aw2ADhGMo3");
    if (!path)
        return MALLOC_ERR;

    if (strcmp(path->parent_dir, "/"))
        return IPFS_ADDR_ERR;

    if (strcmp(path->ipfs_addr, "/ipfs/QmetARxCz9iCcLyTdVCCpbJpJ4jxpTB5FxF4Aw2ADhGMo3"))
        return IPFS_ADDR_ERR;

    if (strcmp(path->mfs_name, "QmetARxCz9iCcLyTdVCCpbJpJ4jxpTB5FxF4Aw2ADhGMo3"))
        return IPFS_ADDR_ERR;

    free(path);
    return NO_ERR;
}

static int parse_ipns_addr() {
    struct mfsf_path* path = mfsf_path_create("/ipns/ipfs.io/test.txt");
    if (!path)
        return MALLOC_ERR;

    if (strcmp(path->parent_dir, "/"))
        return IPNS_ADDR_ERR;

    if (strcmp(path->ipfs_addr, "/ipns/ipfs.io/test.txt"))
        return IPNS_ADDR_ERR;

    if (strcmp(path->mfs_name, "test.txt"))
        return IPNS_ADDR_ERR;

    free(path);
    return NO_ERR;
}

// static int parse_dir_with_ipfs_addr() {
//     struct mfsf_path* path =
//         mfsf_path_create("/home/user/ipfs/QmetARxCz9iCcLyTdVCCpbJpJ4jxpTB5FxF4Aw2ADhGMo3");
//     if (!path)
//         return MALLOC_ERR;
// 
//     if (!path->parent_dir)
//         return DIR_WITH_IPFS_ADDR_ERR;
// 
//     if (strcmp(path->parent_dir, "/home/user"))
//         return DIR_WITH_IPFS_ADDR_ERR;
// 
//     if (strcmp(path->ipfs_addr, "/ipfs/QmetARxCz9iCcLyTdVCCpbJpJ4jxpTB5FxF4Aw2ADhGMo3"))
//         return DIR_WITH_IPFS_ADDR_ERR;
// 
//     if (strcmp(path->mfs_name, "QmetARxCz9iCcLyTdVCCpbJpJ4jxpTB5FxF4Aw2ADhGMo3"))
//         return DIR_WITH_IPFS_ADDR_ERR;
// 
//     free(path);
//     return NO_ERR;
// }

static int parse_non_ipfs_addr() {
    struct mfsf_path* path =
        mfsf_path_create("/home/user/ipfs");
    if (path)
        return NON_IPFS_ADDR_ERR;

    return NO_ERR;
}

int main(int argc, char** argv) {
    int err;

    TEST(parse_empty_addr(), err);
    TEST(parse_ipfs_addr(), err);
    TEST(parse_ipns_addr(), err);
    //TEST(parse_dir_with_ipfs_addr(), err);
    TEST(parse_non_ipfs_addr(), err);

    return err;
}
