#include <string.h>
#include <stdlib.h>

#include "ipfs_operations.c"

#define TEST(x, errno) if ((err = x)) return err

enum ERROR_STATUS {
    MALLOC_ERR = -1,
    NO_ERR,
    RUN_LS_ERR,
};

static int run_ls() {
    union mfsf_result result = mfsf_cmd_run(NULL, "files ls %s", 1, "/");
    return result.result ? RUN_LS_ERR : NO_ERR;
}

int main(int argc, char** argv) {
    int err;

    TEST(run_ls(), err);

    return err;
}
