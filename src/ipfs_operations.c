#include <errno.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ipfs_operations.h"

#define BUF_SIZE 1024

static char* old_cid = NULL;

/* Get the CID of a file in the MFS.
 * Sets errno on error. */
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

/* Generic function for running IPFS commands */
FILE* mfsf_cmd_run(const char* cmd, int argc, ...) {
    va_list va_args;
    int args_len = 0;

    va_start(va_args, argc);
    for (int i = 0; i < argc; i++)
        args_len += strlen(va_arg(va_args, char*)) + 1;
    va_end(va_args);

    int bin_len = strlen(IPFS_BIN);
    int cmd_len = bin_len + strlen(cmd);
    int fmt_len = cmd_len + strlen(" %s") * argc;

    char* fmt_str = malloc(fmt_len + 1);
    char* cmd_str = malloc(cmd_len + args_len + 1);
    if (!(fmt_str && cmd_str)) {
        if (fmt_str)
            free(fmt_str);

        if (cmd_str)
            free(cmd_str);

        errno = ENOMEM;
        return NULL;
    }

    // Create format string
    strcpy(fmt_str, IPFS_BIN);
    strcpy(&fmt_str[bin_len], cmd);
    for (char* ch = &fmt_str[cmd_len]; ch != &fmt_str[fmt_len];) {
        *(ch++) = ' ';
        *(ch++) = '%';
        *(ch++) = 's';
    } fmt_str[fmt_len] = '\0';

    // Create command string
    va_start(va_args, argc);
    vsprintf(cmd_str, fmt_str, va_args);
    va_end(va_args);

    // Run command
    FILE* proc = popen(cmd_str, "r");
    free(fmt_str);
    free(cmd_str);

    return proc;
}

int mfsf_cmd_files_cp(const char* from, const char* to) {
    FILE* proc = mfsf_cmd_run("files cp", 2, from, to);
    if (!proc || pclose(proc) || mfsf_update_pin() || mfsf_publish_path("/"))
        return -errno;

    return 0;
}

int mfsf_cmd_files_mkdir(const char* path) {
    FILE* proc = mfsf_cmd_run("files mkdir", 1, path);
    if (!proc || pclose(proc) || mfsf_update_pin() || mfsf_publish_path("/"))
        return -errno;

    return 0;
}

int mfsf_cmd_files_rm(const char* path, bool recursive) {
    char* cmd = recursive ? "files rm -r" : "files rm";
    FILE* proc = mfsf_cmd_run(cmd, 1, path);
    if (!proc || pclose(proc) || mfsf_update_pin() || mfsf_publish_path("/"))
        return -errno;

    return 0;
}

/* Read the attributes of a file in the MFS and return it's details. */
struct mfsf_stat* mfsf_cmd_files_stat(const char* path) {
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
    // TODO: Improve parsing speed by assuming entry positions or use HTTP API
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

static int handle_pinning(const char* path, const char* pin_cmd) {
    char* cid = cid_from_path(path);
    if (!cid)
        return -errno;

    FILE* proc = mfsf_cmd_run(pin_cmd, 1, cid);
    free(cid);

    if (!proc || pclose(proc))
        return -errno;

    return 0;
}

/* Pin the specified path located in the MFS */
int mfsf_cmd_pin_add(const char* path) {
    return handle_pinning(path, "pin add");
}

/* Unpin the specified path located in the MFS */
int mfsf_cmd_pin_rm(const char* path) {
    return handle_pinning(path, "pin rm");
}

/* Publish the path located in the MFS */
int mfsf_publish_path(const char* path) {
    char* cid = cid_from_path(path);
    if (!cid)
        return -errno;

    FILE* proc = mfsf_cmd_run("name publish", 1, cid);
    free(cid);

    if (!proc || pclose(proc))
        return -errno;

    return 0;
}

void mfsf_update_pin_init() {
    if (!old_cid)
        old_cid = cid_from_path("/");
}

void mfsf_update_pin_destroy() {
    if (old_cid)
        free(old_cid);
}

/* Assuming the root of the MFS is pinned, update it's CID.
 *
 * NOTE: You will have to manually update your MFS root pin if you forget to
 * run `mfsf_update_pin_init()`.
 */
int mfsf_update_pin() {
    if (!old_cid) {
        errno = EFAULT;
        goto pin_update_err;
    }

    char* current_cid = cid_from_path("/");
    if (!current_cid) {
        errno = ENOMEM;
        goto pin_update_err;
    }

    if (!strcmp(old_cid, current_cid)) {
        errno = EFAULT;
        if (old_cid != current_cid)
            free(current_cid);

        goto pin_update_err;
    }

    FILE* proc = mfsf_cmd_run("pin update", 2, old_cid, current_cid);
    if (!proc || pclose(proc)) {
        free(current_cid);
        goto pin_update_err;
    }

    free(old_cid);
    old_cid = current_cid;

    return 0;

    pin_update_err:
        return -errno;
}
