//! 組み込みの標準ヘッダ。`#include <...>` がファイルシステム上で見つからないときだけ使う
//! (`-I` やシステムディレクトリに同名のヘッダがあればそちらが優先される)。
//!
//! 型の大きさは LP64(Linux x86-64)を仮定している。関数は宣言だけで、
//! パーサは関数プロトタイプを読み飛ばす。可変長引数(`<stdarg.h>`)は
//! パーサが対応していないので用意していない。

pub(super) fn builtin_header(name: &str) -> Option<&'static str> {
    Some(match name {
        "stddef.h" => STDDEF,
        "stdbool.h" => STDBOOL,
        "stdint.h" => STDINT,
        "limits.h" => LIMITS,
        "stdnoreturn.h" => STDNORETURN,
        "iso646.h" => ISO646,
        "stdlib.h" => STDLIB,
        "string.h" => STRING,
        "stdio.h" => STDIO,
        "assert.h" => ASSERT,
        _ => return None,
    })
}

const STDDEF: &str = r#"
#ifndef _BUILTIN_STDDEF_H
#define _BUILTIN_STDDEF_H
#define NULL ((void *)0)
typedef unsigned long size_t;
typedef long ptrdiff_t;
typedef int wchar_t;
#define offsetof(type, member) ((size_t)&(((type *)0)->member))
#endif
"#;

const STDBOOL: &str = r#"
#ifndef _BUILTIN_STDBOOL_H
#define _BUILTIN_STDBOOL_H
#define bool _Bool
#define true 1
#define false 0
#define __bool_true_false_are_defined 1
#endif
"#;

const STDINT: &str = r#"
#ifndef _BUILTIN_STDINT_H
#define _BUILTIN_STDINT_H
typedef signed char int8_t;
typedef short int16_t;
typedef int int32_t;
typedef long int64_t;
typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned int uint32_t;
typedef unsigned long uint64_t;
typedef long intptr_t;
typedef unsigned long uintptr_t;
typedef long intmax_t;
typedef unsigned long uintmax_t;
#define INT8_MIN (-128)
#define INT16_MIN (-32768)
#define INT32_MIN (-2147483647 - 1)
#define INT64_MIN (-9223372036854775807L - 1L)
#define INT8_MAX 127
#define INT16_MAX 32767
#define INT32_MAX 2147483647
#define INT64_MAX 9223372036854775807L
#define UINT8_MAX 255
#define UINT16_MAX 65535
#define UINT32_MAX 4294967295U
#define UINT64_MAX 18446744073709551615UL
#define INTPTR_MIN INT64_MIN
#define INTPTR_MAX INT64_MAX
#define UINTPTR_MAX UINT64_MAX
#define INTMAX_MIN INT64_MIN
#define INTMAX_MAX INT64_MAX
#define UINTMAX_MAX UINT64_MAX
#define SIZE_MAX UINT64_MAX
#define INT8_C(c) c
#define INT16_C(c) c
#define INT32_C(c) c
#define INT64_C(c) c ## L
#define UINT8_C(c) c
#define UINT16_C(c) c
#define UINT32_C(c) c ## U
#define UINT64_C(c) c ## UL
#endif
"#;

const LIMITS: &str = r#"
#ifndef _BUILTIN_LIMITS_H
#define _BUILTIN_LIMITS_H
#define CHAR_BIT 8
#define SCHAR_MIN (-128)
#define SCHAR_MAX 127
#define UCHAR_MAX 255
#define CHAR_MIN SCHAR_MIN
#define CHAR_MAX SCHAR_MAX
#define SHRT_MIN (-32768)
#define SHRT_MAX 32767
#define USHRT_MAX 65535
#define INT_MIN (-2147483647 - 1)
#define INT_MAX 2147483647
#define UINT_MAX 4294967295U
#define LONG_MIN (-9223372036854775807L - 1L)
#define LONG_MAX 9223372036854775807L
#define ULONG_MAX 18446744073709551615UL
#define LLONG_MIN (-9223372036854775807LL - 1LL)
#define LLONG_MAX 9223372036854775807LL
#define ULLONG_MAX 18446744073709551615ULL
#endif
"#;

const STDNORETURN: &str = r#"
#ifndef _BUILTIN_STDNORETURN_H
#define _BUILTIN_STDNORETURN_H
#define noreturn _Noreturn
#endif
"#;

const ISO646: &str = r#"
#ifndef _BUILTIN_ISO646_H
#define _BUILTIN_ISO646_H
#define and &&
#define and_eq &=
#define bitand &
#define bitor |
#define compl ~
#define not !
#define not_eq !=
#define or ||
#define or_eq |=
#define xor ^
#define xor_eq ^=
#endif
"#;

const STDLIB: &str = r#"
#ifndef _BUILTIN_STDLIB_H
#define _BUILTIN_STDLIB_H
#include <stddef.h>
#define EXIT_SUCCESS 0
#define EXIT_FAILURE 1
#define RAND_MAX 2147483647
void *malloc(size_t size);
void *calloc(size_t nmemb, size_t size);
void *realloc(void *ptr, size_t size);
void free(void *ptr);
void exit(int status);
void abort(void);
int abs(int n);
long labs(long n);
int atoi(const char *s);
long atol(const char *s);
int rand(void);
void srand(unsigned int seed);
#endif
"#;

const STRING: &str = r#"
#ifndef _BUILTIN_STRING_H
#define _BUILTIN_STRING_H
#include <stddef.h>
void *memcpy(void *dst, const void *src, size_t n);
void *memmove(void *dst, const void *src, size_t n);
void *memset(void *s, int c, size_t n);
int memcmp(const void *a, const void *b, size_t n);
size_t strlen(const char *s);
char *strcpy(char *dst, const char *src);
char *strncpy(char *dst, const char *src, size_t n);
int strcmp(const char *a, const char *b);
int strncmp(const char *a, const char *b, size_t n);
char *strcat(char *dst, const char *src);
char *strchr(const char *s, int c);
#endif
"#;

const STDIO: &str = r#"
#ifndef _BUILTIN_STDIO_H
#define _BUILTIN_STDIO_H
#include <stddef.h>
#define EOF (-1)
struct _IO_FILE;
typedef struct _IO_FILE FILE;
extern FILE *stdin;
extern FILE *stdout;
extern FILE *stderr;
int printf(const char *fmt, ...);
int fprintf(FILE *f, const char *fmt, ...);
int sprintf(char *buf, const char *fmt, ...);
int snprintf(char *buf, size_t n, const char *fmt, ...);
int scanf(const char *fmt, ...);
int puts(const char *s);
int putchar(int c);
int getchar(void);
int fputs(const char *s, FILE *f);
int fputc(int c, FILE *f);
int fflush(FILE *f);
#endif
"#;

const ASSERT: &str = r#"
#undef assert
#ifdef NDEBUG
#define assert(e) ((void)0)
#else
void __assert_fail(const char *expr, const char *file, int line);
#define assert(e) ((e) ? (void)0 : __assert_fail(#e, __FILE__, __LINE__))
#endif
"#;
