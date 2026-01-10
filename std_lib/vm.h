#ifndef VM_STRUCT_H
#define VM_STRUCT_H

/* Generated with cbindgen:0.26.0 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum ArgType {
  Int,
  Str,
  Bool,
  Void,
} ArgType;

typedef union CValue {
  int32_t int_value;
  char *str_value;
  bool bool_value;
  uint8_t void_value;
} CValue;

typedef struct VmArgsValue {
  enum ArgType arg_type;
  union CValue value;
} VmArgsValue;

#endif /* VM_STRUCT_H */
