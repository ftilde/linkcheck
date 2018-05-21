#include "foo.h"
#include <stdio.h>

void foo(int* n) {
    printf("Called foo(&%d)\n", *n);
}
