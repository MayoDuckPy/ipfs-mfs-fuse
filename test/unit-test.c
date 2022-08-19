#include <errno.h>
#include <string.h>
#include <stdlib.h>

// C files are included so static functions may be tested
#include "config.c"
#include "ipfs_operations.c"

#define TEST(x, err) if ((err = x)) return err

enum ERROR_STATUS {
    MALLOC_ERR = -1,
    NO_ERR,
    PARSE_ROOT_ERR,
    RUN_LS_ERR,
};

static int parse_root_dir() {
    /* We can't guarantee having children so this is all we can do */
    struct mfsf_stat* stat = mfsf_cmd_files_stat("/");
    if (!stat || stat->type != MFS_DIRECTORY) {
        free(stat);
        return PARSE_ROOT_ERR;
    }

    free(stat);
    return NO_ERR;
}

static int run_ls() {
    union mfsf_result result = mfsf_cmd_run("files ls %s", 1, NULL, "/");
    return result.result ? RUN_LS_ERR : NO_ERR;
}

int main(int argc, char** argv) {
    if (argc < 2)
        return 0;

    int mfsf_err = 0;

    mfsf_set_config_defaults();
    if (!strcmp(argv[1], "parse_root_dir")) {
        TEST(parse_root_dir(), mfsf_err);
    } else if (!strcmp(argv[1], "run_ls")) {
        TEST(run_ls(), mfsf_err);
    }

    return mfsf_err;
}
