#include <string.h>
#include <stdlib.h>

#include "ipfs_operations.c"

#define TEST(x, errno) if ((err = x)) return err

enum ERROR_STATUS {
    MALLOC_ERR = -1,
    NO_ERR,
    PARSE_ROOT_ERR,
};

static int parse_root_dir() {
    /* We can't guarantee having children so this is all we can do */
    struct mfsf_stat* stat = mfsf_cmd_files_stat("/");
    if (stat->type != MFS_DIRECTORY)
        return PARSE_ROOT_ERR;

    return NO_ERR;
}

int main(int argc, char** argv) {
    int err;

    TEST(parse_root_dir(), err);

    return err;
}
