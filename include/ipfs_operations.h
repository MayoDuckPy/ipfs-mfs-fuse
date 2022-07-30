#ifndef MFS_IPFS_OP_H
#include <stdio.h>

#define MFS_IPFS_OP_H
#define IPFS_BIN "ipfsp" " "
#define CID_MAX  60

FILE* mfsf_cmd_run(const char* cmd, int argc, ...);
#endif
