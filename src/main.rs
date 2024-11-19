mod pak;
mod unpak;
mod metadata;
mod common;
mod view_pak_structure;

use std::sync::atomic::{AtomicBool, Ordering};
use clap::{Parser, Subcommand};
use std::sync::Arc;
use std::io;
use ctrlc;

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
        #[arg(long, short, value_name = "DESCRIPTION", help = "描述信息")]
        description: Option<String>,
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
        #[arg(long, short, value_name = "FILES", help = "是否显示文件列表")]
        files: bool,
    },
    /// 重新计算Metadata
    #[command(arg_required_else_help = true)]
    Update {
        /// 输入文件路径
        #[arg(value_name = "INPUT_FILE")]
        input: String,
        #[arg(long, short, value_name = "DESCRIPTION", help = "更新描述信息")]
        description: Option<String>,
        #[arg(long, short, value_name = "METADATA", help = "更新元数据信息（JSON或Base64编码的JSON）")]
        metadata: Option<String>,
        /// 重新生成所有元数据信息
        #[arg(long, short, help = "重新生成所有元数据信息")]
        all: bool,
    },
    /// 列出包内文件
    #[command(arg_required_else_help = true)]
    List {
        /// 输入文件路径
        #[arg(value_name = "INPUT_FILE")]
        input: String,
        /// 重新扫描文件内容而不是使用metadata
        #[arg(long, short)]
        recheck: bool,
    },
    /// 查看pak结构
    #[command(arg_required_else_help = true, name = "view")]
    ViewStructure {
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
        Commands::Pak { input, output, flat, description, metadata } => {
            pak::pack_files(&input, &output, flat, description.as_deref(), metadata.as_deref(), running)?;
            println!("操作已完成");
        }
        Commands::Unpak { input, output, files } => {
            unpak::unpack_files(&input, &output, files.as_deref(), running)?;
            println!("操作已完成");
        }
        Commands::Metadata { input, files } => {
            metadata::display_metadata(&input, files)?;
        }
        Commands::List { input, recheck } => {
            unpak::list_files(&input, recheck)?;
        }
        Commands::ViewStructure { input } => {
            view_pak_structure::view_structure(&input)?;
        }
        Commands::Update { input, description, metadata, all } => {
            metadata::update_metadata(&input, description.as_deref(), metadata.as_deref(), all)?;
            println!("元数据更新完成");
        }
    }

    Ok(())
}