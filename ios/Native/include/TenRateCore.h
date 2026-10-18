#ifndef TENRATE_CORE_H
#define TENRATE_CORE_H
#include <stdint.h>
char *tenrate_apply(const char *raw, const char *command, int64_t now);
void tenrate_string_free(char *value);
#endif
