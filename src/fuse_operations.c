#include <ctype.h>
#include <errno.h>
#include <time.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "fuse_operations.h"
#include "ipfs_operations.h"

#define BUF_SIZE 1024

static char* name_from_path(const char* path) {
    const char* ch = path;
    for (; *ch != '\0'; ch++);  // Seek to end

    int name_len = 0;
    for (; ch != path && *ch != '/'; ch--, name_len++);

    if (name_len) {
        char* name = malloc(name_len);
        strncpy(name, ++ch, name_len);
        return name;
    }
    return NULL;
}

void mfsf_path_destroy(struct mfsf_path* mfsf_path) {
    if (mfsf_path->parent_dir) {
        if (!strcmp(mfsf_path->parent_dir, "/"))
            mfsf_path->ipfs_addr = NULL;
        free(mfsf_path->parent_dir);
    }

    if (mfsf_path->ipfs_addr)
        free(mfsf_path->ipfs_addr);

    if (mfsf_path)
        free(mfsf_path);
}

struct mfsf_path* mfsf_path_create(const char* path) {
    struct mfsf_path* mfsf_path = calloc(1, sizeof *mfsf_path);
    const char* ipfs_str = "/ipfs/";
    const char* ipns_str = "/ipns/";

    int valid_addr = false;
    int i = 0, type = IPFS_ADDR;
    for (; path[i] != '\0'; i++) {
        if (path[i] == '/') {
            const char* ipfs_start = &path[i];
            if (!strncmp(ipfs_start, ipfs_str, strlen(ipfs_str))
                        || (type = !strncmp(ipfs_start, ipns_str, strlen(ipns_str)))) {
                if (!isalpha(path[i+1]))
                    continue;
                
                if (type)
                    type = IPNS_ADDR;

                valid_addr = true;
                mfsf_path->ipfs_addr = (char*) &path[i];
                break;
            }
        }
    }

    if (!valid_addr) {
        free(mfsf_path);
        return NULL;
    }

    int ipfs_addr_offset = mfsf_path->ipfs_addr - path;
    if (ipfs_addr_offset) {
        mfsf_path->parent_dir = malloc(ipfs_addr_offset + 1);
        strncpy(mfsf_path->parent_dir, path, ipfs_addr_offset);
        mfsf_path->parent_dir[ipfs_addr_offset] = '\0';
        mfsf_path->ipfs_addr = &mfsf_path->parent_dir[ipfs_addr_offset];
    } else {
        mfsf_path->parent_dir = malloc(2);
        mfsf_path->ipfs_addr  = malloc(strlen(path) + 1);

        strcpy(mfsf_path->parent_dir, "/");
        strcpy(mfsf_path->ipfs_addr, path);
    }

    char* ch = mfsf_path->ipfs_addr;
    for (; *ch != '\0'; ch++);  // Seek to end
    for (; ch != mfsf_path->ipfs_addr && *ch != '/'; ch--);
    mfsf_path->mfs_name = ++ch;

    mfsf_path->addr_type = type;
    return mfsf_path;
}

/* Read the attributes of a file from ipfs and return it's details. */
static struct mfsf_stat* parse_ipfs_stat(const char* path) {
    FILE* proc = mfsf_cmd_run("files stat", 1, path);
    struct mfsf_stat* stat = calloc(1, sizeof *stat);
    if (!(proc && stat)) {
        if (proc)
            pclose(proc);

        if (stat) {
            errno = ENOMEM;
            free(stat);
        }

        return NULL;
    }

    // Parse output stats
    // TODO: Improve parsing speed by assuming entry positions
    char buf[BUF_SIZE];
    const char* size_str = "Size: ";
    const char* cumulative_size_str = "CumulativeSize: ";
    const char* children_str = "ChildBlocks: ";
    const char* file_type_str = "Type: ";
    const char* dir_type_str = "directory";
    while (fgets(buf, sizeof buf, proc)) {
        buf[strcspn(buf, "\n")] = '\0';
        if (!strncmp(buf, size_str, strlen(size_str)))
            stat->size = strtol(&buf[strlen(size_str) - 1], NULL, 10);
        else if (!strncmp(buf, cumulative_size_str, strlen(cumulative_size_str)))
            stat->cumulative_size =
                    strtol(&buf[strlen(cumulative_size_str) - 1], NULL, 10);
        else if (!strncmp(buf, children_str, strlen(children_str)))
            stat->children = strtol(&buf[strlen(children_str) - 1], NULL, 10);
        else if (!strncmp(buf, file_type_str, strlen(file_type_str)))
            stat->type = !strcmp(&buf[strlen(file_type_str)], dir_type_str)
                    ? MFS_DIRECTORY : MFS_FILE;
    }

    if (pclose(proc)) {
        errno = ENOENT;  // IPFS returns 1 (EPERM) if no file/directory
        free(stat);
        return NULL;
    }

    return stat;
}

static char* cid_from_path(const char* path) {
    FILE* proc = mfsf_cmd_run("files stat", 1, path);
    char* cid = malloc(CID_MAX);
    if (!(proc && cid)) {
        if (proc)
            pclose(proc);

        if (cid) {
            errno = ENOMEM;
            free(cid);
        }

        return NULL;
    }

    fgets(cid, CID_MAX, proc);
    if (pclose(proc)) {
        free(cid);
        return NULL;
    }

    return cid;
}

/* Publish the current root given by the IPFS files API */
static int publish_name() {
    char* publish_cmd = IPFS_BIN "name publish %s";
    char* cmd = malloc(strlen(publish_cmd) + CID_MAX);

    char* cid = cid_from_path("/");
    if (!cid)
        return -errno;

    sprintf(cmd, publish_cmd, cid);
    FILE* proc = popen(cmd, "r");
    free(cid);
    free(cmd);

    if (!proc || pclose(proc))
        return -errno;

    return 0;
}

/* Pin a specified path given by the IPFS files API */
static int pin_path(const char* path) {
    char* cid = cid_from_path(path);
    if (!cid)
        return -errno;

    FILE* proc = mfsf_cmd_run("pin add", 1, cid);
    free(cid);

    if (!proc || pclose(proc))
        return -errno;

    return 0;
}

int mfsf_getattr(const char* path, struct stat* stat, struct fuse_file_info* fi) {
    struct mfsf_stat* mfs_stat = parse_ipfs_stat(path);
    if (!mfs_stat)
        return -errno;

    struct timespec current_time = { .tv_sec = time(NULL) };
    stat->st_atim  = current_time;
    stat->st_mtim  = stat->st_atim;
    stat->st_ctim  = stat->st_atim;
    stat->st_uid   = getuid();
    stat->st_gid   = getgid();

    if (mfs_stat->type == MFS_DIRECTORY) {
        stat->st_mode = S_IFDIR | 0444;
        stat->st_nlink = mfs_stat->children + 2;
    } else if (mfs_stat->type == MFS_FILE) {
        stat->st_mode = S_IFREG | 0444;
        stat->st_nlink = 1;
        stat->st_size  = mfs_stat->size;
        //stat->st_blocks = mfs_stat->children;
    }

    free(mfs_stat);
    return 0;
}

int mfsf_mkdir(const char* path, mode_t mode) {
    FILE* proc = mfsf_cmd_run("files mkdir", 1, path);
    if (!proc || pclose(proc) || publish_name())
        return -errno;

    return 0;
}

int mfsf_symlink(const char* from, const char* to) {
    // TODO: Remove file on failure
    FILE* proc = mfsf_cmd_run("files cp", 2, from, to);
    if (!proc || pclose(proc) || pin_path(to) || publish_name())
        return -errno;

    return 0;
}

int mfsf_readlink(const char* path, char* buf, size_t size) {
    strncpy(buf, path, size);
    buf[strlen(path) + 1] = '\0';
    return 0;
}

int mfsf_readdir(const char *path, void *buf, fuse_fill_dir_t filler,
        off_t offset, struct fuse_file_info *fi, enum fuse_readdir_flags flags) {

    FILE* proc = mfsf_cmd_run("files ls", 1, path);
    if (!proc)
        return -errno;

    char cmd_out[BUF_SIZE];
    while (fgets(cmd_out, BUF_SIZE, proc)) {
        cmd_out[strcspn(cmd_out, "\n")] = '\0';
        if (filler(buf, cmd_out, NULL, offset, FUSE_FILL_DIR_PLUS))
            break;
    }

    if (pclose(proc))
        return -errno;

    return 0;
}
