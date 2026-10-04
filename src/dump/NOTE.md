
## API
pub fn dump(file_name: &str, section: &str) -> Result<(), DumpError>          // 標準出力へ表示
pub fn dump_to_string(file_name: &str, section: &str) -> Result<String, DumpError> // 文字列で取得
pub fn extract_section(file_name: &str, section: &str) -> Result<SectionData, DumpError> // 生バイト列とアドレスなど