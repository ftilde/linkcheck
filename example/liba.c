#include "liba.h"
#include "libab.h"
#include "libd.h"
#include <stdio.h>

void foo() {
    printf("liba: foo\n");

    d();

    bar();
}
