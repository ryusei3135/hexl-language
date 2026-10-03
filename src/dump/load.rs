use std::fs::File;
use std::io::{self, BufReader, Read};
use std::fs::File;
use std::io::{self, Read};


pub(in crate)
fn byte_emitter(file_name: &str) -> io::Result<()> {
    // 1. バイナリファイルを開く
    let file = File::open(file_name)?;

    // 2. 効率的な読み込みのためにBufReaderでラップする
    let reader = BufReader::new(file);
    const width: usize = 16; // 1行に表示するバイト数

    // 3. bytes()イテレータを使い、1バイトずつ処理する
    for (idx, byte_result) in reader.bytes().enumerate() {
        let byte = byte_result?;
        
        // 16進数（2桁・ゼロ埋め）と文字（制御文字以外）で表示する例
        if byte.is_ascii_graphic() || byte == b' ' {
            println!("0x{:02X} ({})", byte, byte as char);
        } else {
            println!("0x{:02X}", byte);
        }

        // 1行にwidth個のバイトを表示したら改行する
        if (idx + 1) % width == 0 {
            println!();
        }
    }

    Ok(())
}
