linkcheck
=========

linkcheck is a tool for detecting problems with dynamic library resolution of elf files.
Currently, it opens and performs analysis on all library dependencies of the specified file and checks for:

* unresolved symbols
* duplicate symbols
* missing/conflicting libraries

**Limitations**

The analysis of unresolved and duplicate symbols currently yields a number of false positives, i.e. unresolved and duplicate symbols that are not actually problematic.
linkcheck is mostly intended to be used to detect *what* the problem is *if* you have a linking related problem with your project.

**Usage**

```
 > linkcheck --help
linkcheck 0.1.0
ftilde <ftilde@protonmail.com>
Show potential dynamic linking problems of ELF files.

USAGE:
    linkcheck [FLAGS] [OPTIONS] <file>

FLAGS:
    -f, --full analysis         Perform full analysis (default if neither -u, -d, nor -r are specified)
    -h, --help                  Prints help information
    -d, --duplicate-symbols     Show used duplicate symbols
    -r, --lib-resolution        Show library resolution problems
    -u, --unresolved-symbols    Show unresolved symbols
    -V, --version               Prints version information

OPTIONS:
    -l, --lib <search_methods>...    Library search locations (in order specified). Special options are: rpath, runpath,
                                     ld_library_path, ldconfig:<path_to_ld.so.conf>. All other options are interpreted
                                     as fixed paths to library locations. If nothing is specified, the default
                                     resolution behavior of GNU ld.so is mimicked.

ARGS:
    <file>    ELF file to be analyzed

```

**Examples**

Compile the binaries in the `examples` folder and run `linkcheck` on them.

**Licensing**

linkcheck is released under the MIT license.
