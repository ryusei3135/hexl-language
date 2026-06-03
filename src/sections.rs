#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct Elf64_Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}


#[repr(C)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Elf64_Rela {
    r_offset: u64,  // 修正する場所（セクション内オフセット）
    r_info: u64,    // シンボル + タイプ
    r_addend: i64,  // 補正値
}

impl Elf64_Rela {
    pub fn new(offset: u64, info: u64, addend: i64) -> Self {
        Self {
            r_offset: offset,
            r_info: info,
            r_addend: addend,
        }
    }
}


#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Elf64_Shdr {
    pub sh_name: u32,      // セクション名（.shstrtabのオフセット）
    pub sh_type: u32,      // セクションの種類
    pub sh_flags: u64,
    pub sh_addr: u64,
    pub sh_offset: u64,    // ファイル内位置
    pub sh_size: u64,      // セクションサイズ
    pub sh_link: u32,      // 関連セクション
    pub sh_info: u32,      // 追加情報
    pub sh_addralign: u64,
    pub sh_entsize: u64,   // 1エントリサイズ
}


#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Elf64_Sym {
    pub st_name: u32,   // 名前（.strtabのオフセット）
    pub st_info: u8,    // 種類 + バインド
    pub st_other: u8,
    pub st_shndx: u16,  // 所属セクション
    pub st_value: u64,  // セクション内オフセット
    pub st_size: u64,
}

impl Elf64_Sym {
    pub fn make_global_func(name_off: u32, pos: u64) -> Self {
        Self {
            st_name: name_off,
            st_info: 0x12,
            st_other: 0,
            st_shndx: 1,
            st_value: pos,
            st_size: 0,
        }
    }

    pub fn make_undef_func(name_off: u32, pos: u64) -> Self {
        Self {
            st_name: name_off,
            st_info: 0x12,
            st_other: 0,
            st_shndx: 0,
            st_value: pos,
            st_size: 0,
        }
    }    
}


pub const OBJ: u8 = 0x11;

// ================= 定数 =================

const ET_REL: u16 = 1;
const EM_X86_64: u16 = 62;

const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_RELA: u32 = 4;

const SHF_WRITE: u64 = 0x1;
const SHF_ALLOC: u64 = 0x2;





#[derive(Default, Debug, Clone, PartialEq)]
pub struct GenerateBin {
    pub bin: Vec<u8>,
    base_off: u64,
    text: Option<(Vec<u8>, u64)>,
    data: Option<(Vec<u8>, u64)>,
    rela_text: Option<(Vec<Elf64_Rela>, u64)>,
    symtab: Option<(Vec<Elf64_Sym>, u64)>,
    strtab: Option<(Vec<u8>, u64)>,
    shstrtab: Vec<u8>,
    shdrs: u16,
    shoff: u64,
}

impl GenerateBin {
    pub fn new() -> Self {
        Self {
            bin: Vec::new(),
            base_off: size_of::<Elf64_Ehdr>() as u64,
            text: None,
            data: None,
            rela_text: None,
            symtab: None,
            strtab: None,
            shstrtab: Vec::new(),
            shdrs: 0,
            shoff: 0,
        }
    }

    pub fn generate_elf_header(&mut self) {
        let mut e_ident = [0u8; 16];
        e_ident[0..4].copy_from_slice(b"\x7FELF");
        e_ident[4] = 2;
        e_ident[5] = 1;
        e_ident[6] = 1;

        let hdr = Elf64_Ehdr {
            e_ident,
            e_type: ET_REL,
            e_machine: EM_X86_64,
            e_version: 1,
            e_entry: 0,
            e_phoff: 0,
            e_shoff: self.shoff,
            e_flags: 0,
            e_ehsize: size_of::<Elf64_Ehdr>() as u16,
            e_phentsize: 0,
            e_phnum: 0,
            e_shentsize: size_of::<Elf64_Shdr>() as u16,
            e_shnum: self.shdrs + 1,
            e_shstrndx: self.shdrs,
        };
        self.bin[0..as_bytes(&hdr).len()].copy_from_slice(as_bytes(&hdr));
    }

    pub fn setting_text(&mut self, text: Vec<u8>) {
        let text_off: u64 = self.base_off;
        self.base_off += text.len() as u64;
        self.text = Some((text, text_off));
        self.shdrs += 1;
    }

    pub fn setting_data(&mut self, data: Vec<u8>) {
        let data_off: u64 = self.base_off;
        self.base_off += data.len() as u64;
        self.data = Some((data, data_off));
        self.shdrs += 1;
    }

    pub fn setting_strtab(&mut self, strtab: Vec<u8>) {
        let strtab_off: u64 = self.base_off;
        self.base_off += strtab.len() as u64;
        self.strtab = Some((strtab, strtab_off));
        self.shdrs += 1;
    }

    pub fn setting_symtab(&mut self, symtab: Vec<Elf64_Sym>) {
        let symtab_off: u64 = self.base_off;
        self.base_off += (symtab.len() * size_of::<Elf64_Sym>()) as u64;
        self.symtab = Some((symtab, symtab_off));
        self.shdrs += 1;
    }

    pub fn setting_rela_text(&mut self, rela: Vec<Elf64_Rela>) {
        let rela_off: u64 = self.base_off;
        self.base_off += (rela.len() * size_of::<Elf64_Rela>()) as u64;

        self.rela_text = Some((rela, rela_off));
        self.shdrs += 1;
    }

    pub fn insert_codes(&mut self) {
        self.bin.resize(size_of::<Elf64_Ehdr>(), 0);
        self.bin.extend_from_slice(&self.text.clone().unwrap().0);
        self.bin.extend_from_slice(&self.data.clone().unwrap().0);
        let rela = self.rela_text.clone().unwrap().0;
        let symtab = self.symtab.clone().unwrap().0;

        for r in &rela {
            self.bin.extend_from_slice(as_bytes(r));
        }
        for s in &symtab {
            self.bin.extend_from_slice(as_bytes(s));
        }

        self.bin.extend_from_slice(&self.strtab.clone().unwrap().0);
    }

    pub fn generate_shstrtab(&mut self) {
        let mut bin = Vec::new();
        self.shstrtab.push(b'\0');

        bin.extend_from_slice(as_bytes(&Elf64_Shdr::default()));
        
        if let Some((data, off)) = &self.text {
            // .text
            bin.extend_from_slice(as_bytes(&Elf64_Shdr {
                sh_name: self.shstrtab.len() as u32,
                sh_type: SHT_PROGBITS,
                sh_flags: 6,
                sh_addr: 0,
                sh_offset: *off,
                sh_size: data.len() as u64,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 16,
                sh_entsize: 0,
            }));
            self.shstrtab.extend_from_slice(b".text\0");
        }

        if let Some((data, off)) = &self.data {
            // .data
            bin.extend_from_slice(
                as_bytes(&Elf64_Shdr {
                    sh_name: self.shstrtab.len() as u32,
                    sh_type: SHT_PROGBITS,
                    sh_flags: SHF_ALLOC | SHF_WRITE,
                    sh_addr: 0,
                    sh_offset: *off,
                    sh_size: data.len() as u64,
                    sh_link: 0,
                    sh_info: 0,
                    sh_addralign: 1,
                    sh_entsize: 0,
                })
            );
            self.shstrtab.extend_from_slice(b".data\0");
        }

        if let Some((data, off)) = &self.rela_text {
            // .rela.text
            bin.extend_from_slice(as_bytes(&Elf64_Shdr {
                sh_name: self.shstrtab.len() as u32,
                sh_type: SHT_RELA,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: *off,
                sh_size: (data.len() * size_of::<Elf64_Rela>()) as u64,
                sh_link: 4, // symtab
                sh_info: 1, // text
                sh_addralign: 8,
                sh_entsize: size_of::<Elf64_Rela>() as u64,
            }));
            self.shstrtab.extend_from_slice(b".rela.text\0");
        }

        if let Some((data, off)) = &self.symtab {
            // .symtab
            bin.extend_from_slice(as_bytes(&Elf64_Shdr {
                sh_name: self.shstrtab.len() as u32,
                sh_type: SHT_SYMTAB,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: *off,
                sh_size: (data.len() * size_of::<Elf64_Sym>()) as u64,
                sh_link: 5, // strtab
                sh_info: 1,
                sh_addralign: 8,
                sh_entsize: size_of::<Elf64_Sym>() as u64,
            }));
            self.shstrtab.extend_from_slice(b".symtab\0");
        }

        if let Some((data, off)) = &self.strtab {
            // .strtab
            bin.extend_from_slice(as_bytes(&Elf64_Shdr {
                sh_name: self.shstrtab.len() as u32,
                sh_type: SHT_STRTAB,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: *off,
                sh_size: data.len() as u64,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 1,
                sh_entsize: 0,
            }));
            self.shstrtab.extend_from_slice(b".strtab\0");
        }

        let shstrtab_offset = self.bin.len() as u64;

        let pos: u32 = self.shstrtab.len() as u32;
        self.shstrtab.extend_from_slice(b".shstrtab\0");
        // .shstrtab
        bin.extend_from_slice(
            as_bytes(&Elf64_Shdr {
                sh_name: pos,
                sh_type: SHT_STRTAB,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: shstrtab_offset,
                sh_size: self.shstrtab.len() as u64,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 1,
                sh_entsize: 0,
            })
        );
        self.shdrs += 1;
        self.bin.extend_from_slice(&self.shstrtab);
        self.shoff = self.bin.len() as u64;
        self.bin.extend(&bin);
    }
}

fn as_bytes<T>(v: &T) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            (v as *const T) as *const u8,
            size_of::<T>(),
        )
    }
}
