# Function foo is defined (in incompatible ways) in both libfoo.so and libfoo2.so
crash: duplicate_symbol
	./duplicate_symbol

duplicate_symbol: duplicate_symbol.c libfoo2/libfoo2.so libbar/libbar.so
	gcc -g -Wall -o $@ -Wl,-rpath=\$$ORIGIN/libfoo2,-rpath=\$$ORIGIN/libbar -Llibfoo2 -lfoo2 -Llibbar -lbar $@.c

libfoo2/libfoo2.so: libfoo2/foo2.c
	gcc -g -Wall -fpic -shared -o $@ $<

libbar/libbar.so: libbar/bar.c libbar/libfoo/libfoo.so
	gcc -g -Wall -fpic -shared -o $@ -Wl,-rpath=\$$ORIGIN/libfoo -Llibbar/libfoo -lfoo $<

libbar/libfoo/libfoo.so: libbar/libfoo/foo.c
	gcc -g -Wall -fpic -shared -o $@ $<

clean:
	rm -f library_conflict
	find . -name '*.so' -exec rm "{}" \;
