use std::path::PathBuf;
use crate::package::native;
use crate::parse::node;
use crate::package::yaml;
use crate::package::manage;


fn judge_file_extension(filename: &str, extension: &str) -> PathBuf {
    let mut pb = PathBuf::from(filename);
    pb.set_extension(extension);
    pb
}

///  ファイルがyamlか調べる
fn is_yaml_extension(filename: &str) -> Option<PathBuf> {
    for ext in ["yaml", "yml"] {
        let path = judge_file_extension(filename, ext);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

///  ライブラリを使う処理をするノードを実行
pub fn load_native_lib(package_node: node::CalculNode) {
    if package_node.node_type != node::NodeKind::NodeUsePackage {
        eprintln!("[system err]: [file]: package/load.rs");
        panic!("[start node type is not NodeUsePackage]");
    }
    let library_filename = package_node.left_node.clone().unwrap().value;
    let library_dir = package_node.right_node.clone().unwrap().value;

    if let Some(path) = is_yaml_extension(&library_filename) {
        //  yamlファイルの場合
        // yamlファイルを読み込み関数のデータを取得する
        let library_config = yaml::load::load_native_library_config(&path).unwrap();
        //  ネイティブ関数を取得
        let native_module = native::load_exported_functions(
            library_config,
            library_dir.clone()
        );

        if native_module.is_ok() {
            let mut module = native_module.unwrap();
            let module_name = library_filename.split("/").last().unwrap().to_string();
            module.module_name = module_name.clone();

            manage::add_module(module);
        } else {
        }
    }

    println!("{}:{}", library_dir, library_filename);
}
