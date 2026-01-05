use std::fs::File;
use std::error::Error;
use serde::Deserialize;
use std::path::Path;
use serde_yaml::Value;
use serde_yaml;
use crate::package::load::state;


pub struct LoadLibFile {
    pub lib_path: String,
    pub func_datas: Vec<state::FuncData>,
    lib_file_name: String,
}

impl LoadLibFile {
    pub fn new() -> Self {
        Self {
            lib_path: "[*null*]".to_string(),
            func_datas: Vec::<state::FuncData>::new(),
            lib_file_name: "[*null*]".to_string(),
        }
    }

    pub fn load(&mut self, lib_path: String) -> Vec<state::FuncData> {
        if Path::new(&(lib_path.clone() + ".yaml")).exists() {
            self.lib_path = lib_path.clone() + ".yaml";
        } else if Path::new(&(lib_path.clone() + ".yml")).exists() {
            self.lib_path = lib_path.clone() + ".yml";
        }

        self.load_setting_file().unwrap()
    }

    pub fn get_lib_header(&self) -> String {
        self.lib_file_name.clone()
    }

    fn open_file(&self) -> Result<File, std::io::Error> {
        let file = File::open(self.lib_path.clone())?;
        Ok(file)
    }

    //  c言語などで作ったライブラリの設定ファイルを読み込む
    fn load_setting_file(&mut self) -> Result<Vec<state::FuncData>, serde_yaml::Error> {
        let value: Value = serde_yaml::from_reader(self.open_file().unwrap())?;
        let mut func_data: Option<state::FuncData> = None;
        let mut first_key: bool = true;

        if let Some(map) = value.as_mapping() {
            for (key, val) in map {
                let name = key.as_str().unwrap().to_string();
                if first_key && name == "file_name" {
                    self.lib_file_name = val
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap()
                        .to_string();
                    first_key = false;
                    continue;
                }
                let ret = val
                    .get("ret")
                    .and_then(|v| v.as_str())
                    .unwrap()
                    .to_string();
                let args = val
                    .get("args")
                    .and_then(|v| v.as_sequence())
                    .unwrap();
                let mut args_data = Vec::<String>::new();
                for arg in args {
                    args_data
                        .push(
                            arg.as_str().unwrap().to_string()
                        );
                }
                func_data = Some(state::FuncData {
                    func_name: name,
                    args: args_data,
                    ret: ret,
                });

                self.func_datas.push(func_data.clone().unwrap());
            }
        }
        Ok(self.func_datas.clone())
    }
}
