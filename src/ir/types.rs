use super::*;


#[derive(Clone, Debug, PartialEq)]
pub enum Size {
    DB,
    DW,
    DD,
    DQ,
    Struct(Vec<Box<(String, Size)>>),
    Pointer{
        ty:Box<Size>,
        is_const: bool
    },
}

impl Size {
    /// 組み込みの型(byte/u16/int/u64)のみを解決する
    /// 構造体や列挙型などのユーザー定義の型を解決する場合は
    /// `builder::IR::size_of` を使用する
    pub fn new(ty: &node::TyNode) -> Self {
        match ty {
            node::TyNode::Ty(ty) => {
                match ty.as_str() {
                    "byte" => Self::DB,
                    "u16" => Self::DW,
                    "int" => Self::DD,
                    "u64" => Self::DQ,
                    t => panic!("未定義の組み込み型です: {}", t),
                }
            }
            // スタック/静的領域の型は、要素の型と同じサイズを持つ
            node::TyNode::Stack { name, .. } | node::TyNode::Static { name, .. } => {
                Self::new(&node::TyNode::Ty(name.clone()))
            }
            node::TyNode::Pointer { is_const, ty_name } => {
                Self::Pointer{
                    ty: Box::new(Self::new(&*ty_name)),
                    is_const: is_const.clone()
                }
            }
            _ => panic!(),
        }
    }

    pub fn is_pointer(&self) -> Option<Size> {
        if let Self::Pointer { ty, .. } = self {
            Some(*ty.clone())
        } else {
            None
        }
    }

    /// ポインタ型を作成する
    pub fn build_ptr_ty(ty: &node::TyNode) -> Self {
        Self::Pointer {
            ty: Box::new(Self::new(&ty)),
            is_const: false,
        }
    }

    /// 組み込み型かどうかを判定する
    pub fn is_builtin_ty_name(name: &str) -> bool {
        matches!(name, "byte" | "u16" | "int" | "u64")
    }

    /// このサイズがバイト単位で何バイトかを返す
    pub fn to_bytes(&self) -> usize {
        match self {
            Self::DB => 1,
            Self::DW => 2,
            Self::DD => 4,
            Self::DQ => 8,
            _ => panic!(),
        }
    }
}


/// 構造体のバイトサイズを取得する関数
/// アセンブリ言語を生成する際に、サイズが必要だから
pub fn get_struct_size(struct_node: &Vec<inst::MemoryInst>) -> usize {
    let mut size_counter = 0;
    for member in struct_node.iter() {
        match &member {
            inst::MemoryInst::Member{size, ..} => {
                size_counter += size.to_bytes();
            }
            _ => panic!(),
        }
    }
    size_counter
}


