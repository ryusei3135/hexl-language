# src/type.dslから、c言語と連携するための
# 構造体を生成
# 生成後は、src/lib.rsに記述


add_arg_type = """
    Bool,
    Void,
    Str,
"""

add_c_value = """
    pub str_value: *mut c_char,
    pub bool_value: bool,
    pub void_value: u8,
"""

def contains_digit(s: str) -> bool:
    return any(c.isdigit() for c in s)

def main():
    arg_type = "#[repr(C)]\npub enum ArgType {\n"
    c_value = "#[repr(C)]\npub union CValue {\n"

    with open("src/type.dsl", encoding="utf-8") as file:
        for line in file:
            item = line.split(" ")
            if contains_digit(item[0]):
                arg_type += f"    {item[0]},\n"
                c_value += f"    pub {item[1]}_value: {item[1]},\n"

        arg_type += add_arg_type
        c_value += add_c_value

    with open("src/lib.rs", "w") as file:
        file.write(
            "use std::ffi::c_char;\n\n"
            + arg_type + "}\n"
            + c_value + "}\n"
            + """
#[repr(C)]
pub struct VmArgsValue {
    pub arg_type: ArgType,
    pub value: CValue,
}\n"""
        )

main()
