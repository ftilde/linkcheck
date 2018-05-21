# Function foo is defined (in incompatible ways) in both libfoo/libfoo.so (as a direct dependency of library_conflict)
# and libbar/libfoo/libfoo.so (as a dependency of libbar.so)
crash: library_conflict
	./library_conflict

library_conflict: library_conflict.c libfoo/libfoo.so libbar/libbar.so
	gcc -g -Wall -o $@ -Wl,-rpath=\$$ORIGIN/libfoo,-rpath=\$$ORIGIN/libbar -Llibfoo -lfoo -Llibbar -lbar $@.c

libfoo/libfoo.so: libfoo/foo.c
	gcc -g -Wall -fpic -shared -o $@ $<

libbar/libbar.so: libbar/bar.c libbar/libfoo/libfoo.so
	gcc -g -Wall -fpic -shared -o $@ -Wl,-rpath=\$$ORIGIN/libfoo -Llibbar/libfoo -lfoo $<

libbar/libfoo/libfoo.so: libbar/libfoo/foo.c
	gcc -g -Wall -fpic -shared -o $@ $<

clean:
	rm -f library_conflict
	find . -name '*.so' -exec rm "{}" \;
