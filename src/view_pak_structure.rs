use std::io::{self, Read, BufReader};
use std::fs::File;
use console::style;

use crate::common::{MAGIC_NUMBER, MAGIC_METADATA_END, KB, MB, GB};
use crate::metadata::XpakMetadata;

pub fn view_structure(input: &str) -> io::Result<()> {
    let mut pak_file = BufReader::new(File::open(input)?);
    
    // 读取Magic Number
    let mut magic = [0u8; 4];
    pak_file.read_exact(&mut magic)?;
    let magic_valid = &magic == MAGIC_NUMBER;
    
    // 读取metadata长度
    let mut meta_len_bytes = [0u8; 4];
    pak_file.read_exact(&mut meta_len_bytes)?;
    let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;
    
    // 读取metadata内容
    let mut metadata_bytes = vec![0u8; meta_len];
    pak_file.read_exact(&mut metadata_bytes)?;
    let metadata: XpakMetadata = serde_json::from_slice(&metadata_bytes)?;

    // 获取metadata版本
    let metadata_version = metadata.format_version.clone();
    
    // 读取metadata结束标记
    let mut metadata_end = [0u8; 8];
    pak_file.read_exact(&mut metadata_end)?;
    let end_valid = metadata_end == MAGIC_METADATA_END;
    
    // 格式化文件大小显示
    let format_size = |size: u64| -> String {
        if size > GB as u64 {
            format!("{:.2} GB", size as f64 / GB as f64)
        } else if size > MB as u64 {
            format!("{:.2} MB", size as f64 / MB as f64)
        } else if size > KB as u64 {
            format!("{:.2} KB", size as f64 / KB as f64)
        } else {
            format!("{} 字节", size)
        }
    };
    
    println!("\nPAK文件结构分析:");
    println!("┌{:─^100}┐", "");
    
    // Magic Number 部分
    let magic_status = if magic_valid {
        style("✓ 有效").green()
    } else {
        style("X 无效").red()
    };
    println!("│ Magic Number ({:02X?}) {}", magic, magic_status);
    
    // Metadata 部分
    println!("├{:─^100}┤", "");
    println!("│ Metadata 区段: {}", format_size(meta_len as u64));
    println!("│  ├─ Format版本: {}", metadata_version);
    println!("│  ├─ 文件数量: {}", metadata.files_count);
    println!("│  ├─ 总文件大小: {}", format_size(metadata.total_size));
    if let Some(desc) = metadata.description {
        println!("│  └─ 描述: {}", desc);
    }
    
    // Metadata End 部分
    println!("├{:─^100}┤", "");
    let end_status = if end_valid {
        style("✓ 有效").green()
    } else {
        if metadata_version == "1.0" || metadata_version == "1.1" || metadata_version == "1.2" {
            style("O 无效 - 此版本不支持Metadata End").yellow()
        } else {
            style("X 无效 - 数据可能已损坏").red()
        }
    };
    println!("│ Metadata End标记 {:02X?} {}", metadata_end, end_status);
    
    // Data 部分
    println!("├{:─^100}┤", "");
    if end_valid {
        println!("│ Data区段: {}", format_size(metadata.total_size));
        println!("│  └─ 包含 {} 个文件", metadata.files_count);
    } else {
        println!("│ {:<98} │", style("警告：由于Metadata End标记无效或版本不支持，无法确认Data区段的完整性").yellow());
    }
    
    println!("└{:─^100}┘", "");

    Ok(())
}
