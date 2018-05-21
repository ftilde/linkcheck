#include "libfoo/foo.h"
#include "libbar/bar.h"

int main(int argc, char** argv) {
    foo(&argc);
    bar();
    return 0;
}
