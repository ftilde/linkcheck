
all: linktest

linktest: linktest.c liba.so libb.so foobar/libd.so
	gcc -g -Wall -o linktest -Wl,-rpath=\$$ORIGIN/foobar,-rpath=\$$ORIGIN -L. -Lfoobar -ld -lb -la linktest.c

liba.so: subfolder/libab.so liba.c libd.so
	gcc -g -Wall -fpic -shared -o liba.so -Wl,-rpath=\$$ORIGIN,-rpath=\$$ORIGIN/subfolder -L. -Lsubfolder -ld -lab liba.c

subfolder/libab.so: libab.c
	gcc -g -Wall -fpic -shared -o subfolder/libab.so libab.c

libb.so: libb.c
	gcc -g -Wall -fpic -shared -o libb.so libb.c

libd.so: libd1.c libd2.c
	gcc -g -Wall -fpic -shared -o libd.so libd2.c

foobar/libd.so: libd1.c libd2.c
	gcc -g -Wall -fpic -shared -o foobar/libd.so libd1.c

clean:
	rm -f linktest
	rm -f *.so
	rm -f foobar/*.so
	rm -f subfolder/*.so
