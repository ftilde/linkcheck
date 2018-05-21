# We first successfully compile the missing_symbol binary which links to libfoo.so compiled from foo.c.
# After that we compile bar.c (which does not provide the function foo()) and overwrite libfoo.so
# Running missing_symbol results in a symbol lookup error, as libfoo does not provide foo().
run: missing_symbol bar_lib
	@echo Running compiled binary 'missing_symbol':
	./missing_symbol

missing_symbol: missing_symbol.c foo_lib
	gcc -Wall -o $@ -L. -lfoo -Wl,-rpath=\$$ORIGIN $@.c

foo_lib: foo.c
	gcc -Wall -fpic -shared -o libfoo.so foo.c

bar_lib: bar.c
	gcc -Wall -fpic -shared -o libfoo.so bar.c
