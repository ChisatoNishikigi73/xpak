mod pak;
mod unpak;
mod metadata;
mod common;

use clap::{Parser, Subcommand};
use std::io;
use ctrlc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 打包文件或目录
    #[command(arg_required_else_help = true)]
    Pak {
        #[arg(value_name = "INPUT_DIR", help = "要打包的输入目录")]
        input: String,
        #[arg(value_name = "OUTPUT_FILE", help = "打包后的输出文件")]
        output: String,
        #[arg(long, short, value_name = "FLAT", help = "是否扁平化打包（不保留目录结构）")]
        flat: bool,
        #[arg(long, short, value_name = "METADATA", help = "元数据信息（JSON或Base64编码的JSON）")]
        metadata: Option<String>,
    },
    /// 解包文件
    #[command(arg_required_else_help = true)]
    Unpak {
        /// 输入文件路径
        #[arg(value_name = "INPUT_FILE")]
        input: String,
        /// 输出目录路径
        #[arg(value_name = "OUTPUT_DIR")]
        output: String,
        /// 要解包的文件路径列表，如果不指定则解包所有文件
        #[arg(long, short, num_args = 1.., value_name = "FILES")]
        files: Option<Vec<String>>,
    },
    /// 查看元数据信息
    #[command(arg_required_else_help = true)]
    Metadata {
        /// 输入文件路径
        #[arg(value_name = "INPUT_FILE")]
        input: String,
    },
    /// 列出包内文件
    #[command(arg_required_else_help = true)]
    List {
        /// 输入文件路径
        #[arg(value_name = "INPUT_FILE")]
        input: String,
    },
}

fn main() -> io::Result<()> {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));  
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        println!("\n操作已取消");
    }).expect("无法设置 Ctrl-C 处理器");

    let cli = Cli::parse();

    match cli.command {
        Commands::Pak { input, output, flat, metadata } => {
            pak::pack_files(&input, &output, flat, metadata.as_deref(), running)?;
            println!("操作已完成");
        }
        Commands::Unpak { input, output, files } => {
            unpak::unpack_files(&input, &output, files.as_deref(), running)?;
            println!("操作已完成");
        }
        Commands::Metadata { input } => {
            metadata::display_metadata(&input)?;
        }
        Commands::List { input } => {
            unpak::list_files(&input)?;
        }
    }

    Ok(())
}