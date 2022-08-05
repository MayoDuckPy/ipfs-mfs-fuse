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
    union mfsf_result result = mfsf_cmd_run("r", "files stat \"%s\"", 1, path);
    char* cid = malloc(CID_MAX);
    if (!(result.stream && cid)) {
        if (result.stream)
            pclose(result.stream);

        if (cid) {
            errno = ENOMEM;
            free(cid);
        }

        return NULL;
    }

    fgets(cid, CID_MAX, result.stream);
    if (pclose(result.stream)) {
        free(cid);
        return NULL;
    }

    cid[strcspn(cid, "\n")] = '\0';
    return cid;
}

/* Generic function for running IPFS commands.
 * @param pipe_type The type of stream that should be opened by the command.
 *                  Otherwise, no stream will be opened if NULL.
 * @param cmd A printf-like format string of the command to be passed to IPFS.
 * @param argc The number of arguments.
 */
union mfsf_result mfsf_cmd_run(const char* pipe_type, const char* cmd, int argc, ...) {
    union mfsf_result result = {0};
    va_list va_args;
    int args_len = 0;

    va_start(va_args, argc);
    for (int i = 0; i < argc; i++)
        args_len += strlen(va_arg(va_args, char*));
    va_end(va_args);

    char* cmd_str = malloc(strlen(IPFS_BIN) + strlen(cmd) + args_len);
    if (!cmd_str) {
        errno = ENOMEM;
        return result;
    }

    // Create command string
    va_start(va_args, argc);
    strcpy(cmd_str, IPFS_BIN);
    vsprintf(&cmd_str[strlen(IPFS_BIN)], cmd, va_args);
    va_end(va_args);

    // Run command
    if (pipe_type)
        result.stream = popen(cmd_str, pipe_type);
    else
        result.result = system(cmd_str);

    free(cmd_str);
    return result;
}

int mfsf_cmd_files_cp(const char* from, const char* to) {
    union mfsf_result result = mfsf_cmd_run(NULL, "files cp \"%s\" \"%s\"", 2, from, to);
    if (result.result || mfsf_update_pin() || mfsf_publish_path("/"))
        return -errno;

    return 0;
}

int mfsf_cmd_files_mkdir(const char* path) {
    union mfsf_result result = mfsf_cmd_run(NULL, "files mkdir \"%s\"", 1, path);
    if (result.result || mfsf_update_pin() || mfsf_publish_path("/"))
        return -errno;

    return 0;
}

int mfsf_cmd_files_rm(const char* path, bool recursive) {
    char* cmd = recursive ? "files rm -r \"%s\"" : "files rm \"%s\"";
    union mfsf_result result = mfsf_cmd_run(NULL, cmd, 1, path);
    if (result.result || mfsf_update_pin() || mfsf_publish_path("/"))
        return -errno;

    return 0;
}

/* Read the attributes of a file in the MFS and return it's details. */
struct mfsf_stat* mfsf_cmd_files_stat(const char* path) {
    union mfsf_result result = mfsf_cmd_run("r", "files stat \"%s\"", 1, path);
    struct mfsf_stat* stat = calloc(1, sizeof *stat);
    if (!(result.stream && stat)) {
        if (result.stream)
            pclose(result.stream);

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
    while (fgets(buf, sizeof buf, result.stream)) {
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

    if (pclose(result.stream)) {
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

    union mfsf_result result = mfsf_cmd_run(NULL, pin_cmd, 1, cid);
    free(cid);

    if (result.result)
        return -errno;

    return 0;
}

/* Pin the specified path located in the MFS */
int mfsf_cmd_pin_add(const char* path) {
    return handle_pinning(path, "pin add \"%s\"");
}

/* Unpin the specified path located in the MFS */
int mfsf_cmd_pin_rm(const char* path) {
    return handle_pinning(path, "pin rm \"%s\"");
}

/* Publish the path located in the MFS */
int mfsf_publish_path(const char* path) {
    char* cid = cid_from_path(path);
    if (!cid)
        return -errno;

    union mfsf_result result = mfsf_cmd_run(NULL, "name publish --allow-offline %s", 1, cid);
    free(cid);

    if (result.result)
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

    union mfsf_result result = mfsf_cmd_run(NULL, "pin update %s %s", 2, old_cid, current_cid);
    if (result.result) {
        free(current_cid);
        goto pin_update_err;
    }

    free(old_cid);
    old_cid = current_cid;

    return 0;

    pin_update_err:
        return -errno;
}
