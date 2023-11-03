#![cfg_attr(feature = "axstd", no_std)]
// 如果名为 "axstd" 的特性（feature）被启用（通常在Cargo.toml 中配置），则在编译时将禁用标准库（no_std）
#![cfg_attr(feature = "axstd", no_main)]
// 如果名为 "axstd" 的特性被启用，将禁用 main 函数（no_main）

#[macro_use]
// `#[macro_use]`的主要作用是使宏在当前 crate 的作用域中可见，从而允许你在代码中使用这些宏，而无需显式导入它们。
#[cfg(feature = "axstd")]
extern crate axstd as std;
// 编译器要根据是否启用 "axstd" 特性来配置代码
// 在启用 "axstd" 特性的情况下，你的代码将使用 "axstd" crate 作为标准库，而不是 Rust 标准库。

macro_rules! path_to_str {
    ($path:expr) => {{
        #[cfg(not(feature = "axstd"))]
        {
            $path.to_str().unwrap() // Path/OsString -> &str
        }
        #[cfg(feature = "axstd")]
        {
            $path.as_str() // String -> &str
        }
    }};
}

mod cmd;

#[cfg(feature = "use-ramfs")]
mod ramfs;

use std::io::prelude::*;

const LF: u8 = b'\n';
const CR: u8 = b'\r';
const DL: u8 = b'\x7f';
const BS: u8 = b'\x08';
const SPACE: u8 = b' ';

const MAX_CMD_LEN: usize = 256;

fn print_prompt() {
    print!(
        "arceos:{}$ ",
        path_to_str!(std::env::current_dir().unwrap())
    );
    std::io::stdout().flush().unwrap();
}

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    let mut stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    let mut buf = [0; MAX_CMD_LEN];
    let mut cursor = 0;
    cmd::run_cmd("help".as_bytes());
    print_prompt();

    loop {
        if stdin.read(&mut buf[cursor..cursor + 1]).ok() != Some(1) {
            continue;
        }
        if buf[cursor] == b'\x1b' {
            buf[cursor] = b'^';
        }
        match buf[cursor] {
            CR | LF => {
                println!();
                if cursor > 0 {
                    cmd::run_cmd(&buf[..cursor]);
                    cursor = 0;
                }
                print_prompt();
            }
            BS | DL => {
                if cursor > 0 {
                    stdout.write_all(&[BS, SPACE, BS]).unwrap();
                    cursor -= 1;
                }
            }
            0..=31 => {}
            c => {
                if cursor < MAX_CMD_LEN - 1 {
                    stdout.write_all(&[c]).unwrap();
                    cursor += 1;
                }
            }
        }
    }
}
