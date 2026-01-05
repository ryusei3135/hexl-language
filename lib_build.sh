CC=gcc
DIR=std_lib
BUILD_DIR=lib/std

mkdir lib/std/

$CC -fPIC -shared $DIR/io.c -o $BUILD_DIR/io.so
