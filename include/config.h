#ifndef MFSF_CONFIG_H
#define MFSF_CONFIG_H
/* Configuration TODOs:
 *
 * Toggle HTTP API usage
 * HTTP API address
 */
struct mfsf_config {
    char* ipfs_bin;
    char* ipfs_path;
    unsigned int cid_ver;
};
struct mfsf_config* mfsf_get_config();
struct fuse_opt* mfsf_get_options();
void mfsf_set_config_defaults();
#endif
