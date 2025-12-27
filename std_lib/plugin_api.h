// plugin_api.h
#ifndef PLUGIN_API_H
#define PLUGIN_API_H

#include <stdint.h>

#define PLUGIN_API_VERSION 1

typedef int32_t (*plugin_func_t)(const char* input);

typedef struct {
    const char* name;
    plugin_func_t func;
} plugin_entry_t;

typedef struct {
    uint32_t api_version;
    const plugin_entry_t* entries;
    uint32_t entry_count;
} plugin_api_t;

const plugin_api_t* plugin_get_api(void);


#endif