#include "libfoo2/foo2.h"
#include "libbar/bar.h"

int main(int argc, char** argv) {
    foo(&argc);
    bar();
    return 0;
}
