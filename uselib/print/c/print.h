#pragma once


typedef unsigned char       u8;
typedef unsigned short      u16;
typedef unsigned int        u32;
typedef unsigned long long  u64;
typedef long long           i64;
typedef unsigned long long  uptr;   /* x86-64 上のポインタ幅整数 */


long linux_sys_write(const char *, unsigned long);
