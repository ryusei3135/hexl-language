# 標準ライブラリをビルド

CC=clang++
DIR=std_lib
BUILD_DIR=extern_lib/std

rm -r extern_lib
mkdir extern_lib
mkdir $BUILD_DIR
cp $DIR/io.yaml $BUILD_DIR/io.yaml

$CC -fPIC -shared $DIR/io.cpp -o $BUILD_DIR/io.so
