use std::fs::{self, File, FileType};
use std::io::{self, prelude::*};
use std::{string::String, vec::Vec};

#[cfg(all(not(feature = "axstd"), unix))]
use std::os::unix::fs::{FileTypeExt, PermissionsExt};

macro_rules! print_err {
    ($cmd: literal, $msg: expr) => {
        println!("{}: {}", $cmd, $msg);
    };
    ($cmd: literal, $arg: expr, $err: expr) => {
        println!("{}: {}: {}", $cmd, $arg, $err);
    };
}

type CmdHandler = fn(&str);

const CMD_TABLE: &[(&str, CmdHandler)] = &[
    ("cat", do_cat),
    ("cd", do_cd),
    ("echo", do_echo),
    ("exit", do_exit),
    ("help", do_help),
    ("ls", do_ls),
    ("mkdir", do_mkdir),
    ("pwd", do_pwd),
    ("rm", do_rm),
    ("uname", do_uname),
    ("rename", do_rename),
    ("mv", do_move)
];

fn file_type_to_char(ty: FileType) -> char {
    if ty.is_char_device() {
        'c'
    } else if ty.is_block_device() {
        'b'
    } else if ty.is_socket() {
        's'
    } else if ty.is_fifo() {
        'p'
    } else if ty.is_symlink() {
        'l'
    } else if ty.is_dir() {
        'd'
    } else if ty.is_file() {
        '-'
    } else {
        '?'
    }
}

#[rustfmt::skip]
// 跳过对标记的代码段的自动格式化处理
const fn file_perm_to_rwx(mode: u32) -> [u8; 9] {
    // 常量函数是在编译时计算结果并被嵌入到生成的代码中的函数。
    let mut perm = [b'-'; 9];
    macro_rules! set {
        ($bit:literal, $rwx:literal) => {
            if mode & (1 << $bit) != 0 {
                perm[8 - $bit] = $rwx
            }
        };
    }

    set!(2, b'r'); set!(1, b'w'); set!(0, b'x');
    set!(5, b'r'); set!(4, b'w'); set!(3, b'x');
    set!(8, b'r'); set!(7, b'w'); set!(6, b'x');
    perm
}

fn do_ls(args: &str) {
    let current_dir = std::env::current_dir().unwrap();
    let args = if args.is_empty() {
        path_to_str!(current_dir)
    } else {
        args
    };
    let name_count = args.split_whitespace().count();

    fn show_entry_info(path: &str, entry: &str) -> io::Result<()> {
        let metadata = fs::metadata(path)?;
        let size = metadata.len();
        let file_type = metadata.file_type();
        let file_type_char = file_type_to_char(file_type);
        let rwx = file_perm_to_rwx(metadata.permissions().mode());
        let rwx = unsafe { core::str::from_utf8_unchecked(&rwx) };
        println!("{}{} {:>8} {}", file_type_char, rwx, size, entry);
        Ok(())
    }

    fn list_one(name: &str, print_name: bool) -> io::Result<()> {
        let is_dir = fs::metadata(name)?.is_dir();
        if !is_dir {
            return show_entry_info(name, name);
        }

        if print_name {
            println!("{}:", name);
        }
        let mut entries = fs::read_dir(name)?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect::<Vec<_>>();
        entries.sort();

        for entry in entries {
            let entry = path_to_str!(entry);
            let path = String::from(name) + "/" + entry;
            if let Err(e) = show_entry_info(&path, entry) {
                print_err!("ls", path, e);
            }
        }
        Ok(())
    }

    for (i, name) in args.split_whitespace().enumerate() {
        if i > 0 {
            println!();
        }
        if let Err(e) = list_one(name, name_count > 1) {
            print_err!("ls", name, e);
        }
    }
}

fn do_cat(args: &str) {
    if args.is_empty() {
        print_err!("cat", "no file specified");
        return;
    }

    fn cat_one(fname: &str) -> io::Result<()> {
        let mut buf = [0; 1024];
        let mut file = File::open(fname)?;
        loop {
            let n = file.read(&mut buf)?;
            if n > 0 {
                io::stdout().write_all(&buf[..n])?;
            } else {
                return Ok(());
            }
        }
    }

    for fname in args.split_whitespace() {
        if let Err(e) = cat_one(fname) {
            print_err!("cat", fname, e);
        }
    }
}

fn do_echo(args: &str) {
    fn echo_file(fname: &str, text_list: &[&str]) -> io::Result<()> {
        let mut file = File::create(fname)?;
        for text in text_list {
            file.write_all(text.as_bytes())?;
        }
        Ok(())
    }

    if let Some(pos) = args.rfind('>') {
        let text_before = args[..pos].trim();
        let (fname, text_after) = split_whitespace(&args[pos + 1..]);
        if fname.is_empty() {
            print_err!("echo", "no file specified");
            return;
        };

        let text_list = [
            text_before,
            if !text_after.is_empty() { " " } else { "" },
            text_after,
            "\n",
        ];
        if let Err(e) = echo_file(fname, &text_list) {
            print_err!("echo", fname, e);
        }
    } else {
        println!("{}", args)
    }
}

fn do_mkdir(args: &str) {
    if args.is_empty() {
        print_err!("mkdir", "missing operand");
        return;
    }

    fn mkdir_one(path: &str) -> io::Result<()> {
        fs::create_dir(path)
    }

    for path in args.split_whitespace() {
        if let Err(e) = mkdir_one(path) {
            print_err!("mkdir", format_args!("cannot create directory '{path}'"), e);
        }
    }
}

fn do_rm(args: &str) {
    if args.is_empty() {
        print_err!("rm", "missing operand");
        return;
    }
    let mut rm_dir = false;
    for arg in args.split_whitespace() {
        if arg == "-d" {
            rm_dir = true;
        }
    }

    fn rm_one(path: &str, rm_dir: bool) -> io::Result<()> {
        if rm_dir && fs::metadata(path)?.is_dir() {
            fs::remove_dir(path)
        } else {
            fs::remove_file(path)
        }
    }

    for path in args.split_whitespace() {
        if path == "-d" {
            continue;
        }
        if let Err(e) = rm_one(path, rm_dir) {
            print_err!("rm", format_args!("cannot remove '{path}'"), e);
        }
    }
}

fn do_cd(mut args: &str) {
    if args.is_empty() {
        args = "/";
    }
    if !args.contains(char::is_whitespace) {
        if let Err(e) = std::env::set_current_dir(args) {
            print_err!("cd", args, e);
        }
    } else {
        print_err!("cd", "too many arguments");
    }
}

fn do_pwd(_args: &str) {
    let pwd = std::env::current_dir().unwrap();
    println!("{}", path_to_str!(pwd));
}

fn do_uname(_args: &str) {
    let arch = option_env!("AX_ARCH").unwrap_or("");
    let platform = option_env!("AX_PLATFORM").unwrap_or("");
    let smp = match option_env!("AX_SMP") {
        None | Some("1") => "",
        _ => " SMP",
    };
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("0.1.0");
    println!(
        "ArceOS {ver}{smp} {arch} {plat}",
        ver = version,
        smp = smp,
        arch = arch,
        plat = platform,
    );
}

fn do_help(_args: &str) {
    println!("Available commands:");
    for (name, _) in CMD_TABLE {
        println!("  {}", name);
    }
}

fn do_exit(_args: &str) {
    println!("Bye~");
    std::process::exit(0);
}

// 测试样例
// make A=apps/fs/shell ARCH=x86_64 LOG=info SMP=4 run BLK=y DISK_IMG=./disk/disk.img
// rename /Program/arceos.txt arce.txt
// rename short.txt test.txt
// rename test.txt short.txt
// rename java.txt short.txt
// rename tests /Program/tests 
// rename -d /Program/tests /Program/test
fn do_rename(args: &str) {
    let mut _rn_dir = false;
    let mut file_names: Vec<&str> = Vec::new();
    for arg in args.split_whitespace() {
        if arg == "-d" {
            _rn_dir = true;
        } else {
            file_names.push(arg);
        }
    }

    if let Err(_) = fs::metadata(file_names[0]) {
        print_err!("move", "value NotFound");
        return ;
    }

    if file_names.len() == 2 {
        fs::rename(file_names[0], file_names[1]).unwrap();
    } else if file_names.len() > 2 {
        print_err!("rename", "Too many arguments");
    } else {
        print_err!("rename", "Too few arguments");
    }
}

// 重命名文件：
// 如果newname指定的文件存在，则会被删除。
// 如果newname与oldname不在一个目录下，则相当于移动文件。
// 重命名目录：
// 如果newname和oldname都为目录，则重命名目录。
// 如果newname指定的目录存在且为空目录，则先将newname删除。
// 对于newname和oldname两个目录，调用进程必须有写权限。
// 重命名目录时，newname不能包含oldname作为其路径前缀。例如，不能将/usr更名为/usr/foo/testdir，因为老名字（ /usr/foo）是新名字的路径前缀，因而不能将其删除。

// 测试样例
// cd Program
// mv /Program/Rust/test.txt /Program
// mv /Program/test.txt /Program/Rust
// mv /Program/java.txt /Program/Rust

// mv -d /Program/Rust/JavaScript /Program
// mv -d /Program/JavaScript /Program/Rust
// mv -d /Program/Java /Program/Rust
fn do_move(args: &str) {
    let mut _mv_dir = false;
    let mut file_names: Vec<&str> = Vec::new();
    for arg in args.split_whitespace() {
        if arg == "-d" {
            _mv_dir = true;
        } else {
            file_names.push(arg);
        }
    }
    for file_meta in file_names.iter() {
        if let Err(_) = fs::metadata(file_meta) {
            print_err!("move", "value NotFound");
            return ;
        }
    }
    // println!("{}", mv_dir);
    fn move_one(file_names: Vec<&str>) {
        if fs::metadata(file_names[1]).unwrap().is_dir() {
            println!("{:?}", file_names);
            let file_name = &file_names[0][file_names[0].rfind('/').unwrap()+1..];
            let mut new_file = String::from(file_names[1]);
            new_file.push_str("/");
            new_file.push_str(file_name);
            // println!("{}", file_names[0]);
            // println!("{}", new_file);
            fs::rename(file_names[0], &new_file).unwrap();
        } else {
            print_err!("move", "The output must be a folder");
        }
    }

    if file_names.len() == 2 {
        move_one(file_names)
    } else if file_names.len() > 2 {
        print_err!("move", "Too many arguments");
    } else {
        print_err!("move", "Too few arguments");
    }
}
    
pub fn run_cmd(line: &[u8]) {
    let line_str = unsafe { core::str::from_utf8_unchecked(line) };
    let (cmd, args) = split_whitespace(line_str);
    if !cmd.is_empty() {
        for (name, func) in CMD_TABLE {
            if cmd == *name {
                func(args);
                return;
            }
        }
        println!("{}: command not found", cmd);
    }
}

fn split_whitespace(str: &str) -> (&str, &str) {
    let str = str.trim();
    str.find(char::is_whitespace)
        .map_or((str, ""), |n| (&str[..n], str[n + 1..].trim()))
}
