use libloading::{Library, Symbol};
use std::fs::File;
use std::error::Error;
use serde::Deserialize;
use std::path::Path;
use crate::parse::node;
use crate::package::state::*;
use crate::package::manager;

mod file;
pub mod state;



fn make_lib_data_to_func(
    lib_data: &mut file::LoadLibFile,
    path: String,
    src_path: String
) -> Result<(), Box<dyn std::error::Error>> {
    //  ライブラリの中にある関数の情報
    let func_datas = lib_data.load(path);
    unsafe {
        let lib = Library::new(
            &(
                src_path.as_str().to_owned()
                + lib_data.get_lib_header().as_str()
            )
        ).expect("this lib is not found");

        for data in func_datas {
            let func_name = data.func_name.clone();
            let func: Symbol<CFunc> = lib.get(func_name.as_bytes())?;
            manager::add_c_func(func_name, func, data.clone());
        }
    }
    Ok(())
}

pub fn load_lib(package_node: node::CalculNode) {
    if package_node.node_type == node::NodeKind::NodeUsePackage {
        let dir = package_node.left_node.unwrap().clone();
        let src = package_node.right_node.unwrap().clone();
        let path = dir.value;

        let mut lib = file::LoadLibFile::new();
        make_lib_data_to_func(&mut lib, path, src.value.clone());
    } else {
        println!("[system err]: [file]: package/load.rs");
        println!("[func]: load_lib");
        panic!("The passed node is not 'NodeUsePackage'");
    }
}
