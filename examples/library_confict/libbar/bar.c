#include "bar.h"
#include "libfoo/foo.h"
#include <stdio.h>

void bar() {
    printf("Called bar\n");

    foo();
}
