#ifndef VM_STRUCT_H
#define VM_STRUCT_H

/* Generated with cbindgen:0.26.0 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum ArgType {
  Int32,
  Float32,
  Bool,
  Void,
  Str,
} ArgType;

typedef union CValue {
  int32_t i32_value;
  float f32_value;
  char *str_value;
  bool bool_value;
  uint8_t void_value;
} CValue;

typedef struct VmArgsValue {
  enum ArgType arg_type;
  union CValue value;
} VmArgsValue;

#endif /* VM_STRUCT_H */
