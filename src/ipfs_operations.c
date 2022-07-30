#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ipfs_operations.h"

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
