#include "stdlib.h"

int range(char *end) {
    static int start = 0;

    if (start < atoi(end)) {
        start++;
    }
    return start;
}
