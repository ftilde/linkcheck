# We first successfully compile the example binary which links to libfoo.so compiled from foo.c.
# After that we compile bar.c (which does not provide the function foo()) and overwrite libfoo.so
# Running example results in a symbol lookup error, as libfoo does not provide foo().
run: example bar_lib
	@echo Running compiled binary 'example':
	./example

example: example.c foo_lib
	gcc -Wall -o $@ -L. -lfoo -Wl,-rpath=\$$ORIGIN $@.c

foo_lib: foo.c
	gcc -Wall -fpic -shared -o libfoo.so foo.c

bar_lib: bar.c
	gcc -Wall -fpic -shared -o libfoo.so bar.c
