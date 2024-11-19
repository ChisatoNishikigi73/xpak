use std::io::{self, Read, Write, BufReader, BufWriter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs::{self, File};
use std::path::Path;
use std::sync::Arc;
use serde_json;
use indicatif::{ProgressBar, ProgressStyle};

use crate::common::{BUFFER_SIZE, GB, KB, MAGIC_METADATA_END, MAGIC_NUMBER, MB};
use crate::metadata::XpakMetadata;

pub fn unpack_files(
    input: &str, 
    output: &str, 
    selected_files: Option<&[String]>,
    running: Arc<AtomicBool>
) -> io::Result<()> {
    let output_path = Path::new(output);
    fs::create_dir_all(output_path)?;

    let mut pak_file = BufReader::with_capacity(BUFFER_SIZE, File::open(input)?);

    // 验证Magic Number
    let mut magic = [0u8; 4];
    pak_file.read_exact(&mut magic)?;
    if &magic != MAGIC_NUMBER {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "无效的文件格式"));
    }

    // 读取metadata
    let mut meta_len_bytes = [0u8; 4];
    pak_file.read_exact(&mut meta_len_bytes)?;
    let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;
    
    let mut metadata_bytes = vec![0u8; meta_len];
    pak_file.read_exact(&mut metadata_bytes)?;
    
    // 验证metadata结束标记
    let mut metadata_end = [0u8; 8];
    pak_file.read_exact(&mut metadata_end)?;
    if metadata_end != MAGIC_METADATA_END {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "无效的metadata结束标记"));
    }

    let metadata: XpakMetadata = serde_json::from_slice(&metadata_bytes)?;
    
    // 读取文件数量
    let mut count_bytes = [0u8; 4];
    pak_file.read_exact(&mut count_bytes)?;
    let count = u32::from_le_bytes(count_bytes);

    if count != metadata.files_count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("文件数量不匹配：metadata中为{}，实际为{}", metadata.files_count, count)
        ));
    }

    // 创建进度条
    let progress = ProgressBar::new(metadata.total_size);
    progress.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("#>-"));

    // 预分配缓冲区
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut files_unpacked = 0;

    for _ in 0..count {
        if !running.load(Ordering::SeqCst) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "操作被用户取消"));
        }

        // 读取文件路径
        let mut path_len_bytes = [0u8; 4];
        pak_file.read_exact(&mut path_len_bytes)?;
        let path_len = u32::from_le_bytes(path_len_bytes) as usize;

        let mut path_bytes = vec![0u8; path_len];
        pak_file.read_exact(&mut path_bytes)?;
        let path_str = String::from_utf8(path_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // 读取文件大小
        let mut content_len_bytes = [0u8; 4];
        pak_file.read_exact(&mut content_len_bytes)?;
        let content_len = u32::from_le_bytes(content_len_bytes) as usize;

        // 检查是否需要解包此文件
        if selected_files.map_or(true, |files| files.iter().any(|f| path_str == *f)) {
            let file_path = output_path.join(&path_str);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let file = File::create(&file_path)?;
            let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);

            if content_len >= BUFFER_SIZE {
                // 大文件使用 io::copy
                let mut limited_reader = (&mut pak_file).take(content_len as u64);
                io::copy(&mut limited_reader, &mut writer)?;
            } else {
                // 小文件使用缓冲区
                let mut remaining = content_len;
                while remaining > 0 {
                    let to_read = remaining.min(BUFFER_SIZE);
                    let buf = &mut buffer[..to_read];
                    pak_file.read_exact(buf)?;
                    writer.write_all(buf)?;
                    remaining -= to_read;
                }
            }

            writer.flush()?;
            files_unpacked += 1;
        } else {
            // 跳过不需要的文件
            pak_file.seek_relative(content_len as i64)?;
        }

        progress.inc(content_len as u64);
    }

    progress.finish();
    println!("共解包 {} 个文件", files_unpacked);
    Ok(())
}

pub fn list_files(input: &str, recheck: bool) -> io::Result<()> {
    if !recheck {
        // 快速模式：只读取metadata
        let file = File::open(input)?;
        let mut pak_file = BufReader::with_capacity(BUFFER_SIZE, file);

        // 验证Magic Number
        let mut magic = [0u8; 4];
        pak_file.read_exact(&mut magic)?;
        if &magic != MAGIC_NUMBER {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "无效的文件格式"));
        }

        // 读取metadata
        let mut meta_len_bytes = [0u8; 4];
        pak_file.read_exact(&mut meta_len_bytes)?;
        let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;
        if meta_len > 0 {
            let mut metadata_bytes = vec![0u8; meta_len];
            pak_file.read_exact(&mut metadata_bytes)?;
            let metadata = match serde_json::from_slice::<XpakMetadata>(&metadata_bytes) {
                Ok(metadata) => metadata,
                Err(e) => {
                    println!("警告：无法从metadata读取文件列表，切换完扫描模式");
                    println!("错误信息: {}", e);
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "无法解析metadata"));
                }
            };
            println!("文件列表 ({} 个文件):", metadata.files_count);
            println!("----------------------------------------");
                
            for (i, file) in metadata.files.iter().enumerate() {
                println!("{:4}. {} ({} 字节)", i + 1, file.path, file.size);
            }
            
            let total_size = metadata.total_size + meta_len as u64;
            if total_size > GB as u64 {
                println!("总大小: {} GB", total_size / GB as u64);
            } else if total_size > MB as u64 {
                println!("总大小: {} MB", total_size / MB as u64);
            } else if total_size > KB as u64 {
                println!("总大小: {} KB", total_size / KB as u64);
            } else {
                println!("总大小: {} 字节", total_size);
            }
            println!("├Metadata长度: {} 字节", meta_len);
            println!("└─文件大小: {} 字节", metadata.total_size);

            return Ok(());
        }
    }
    
    // 完整扫描模式
    let mut pak_file = BufReader::new(File::open(input)?);
    
    // 验证Magic Number
    let mut magic = [0u8; 4];
    pak_file.read_exact(&mut magic)?;
    if &magic != MAGIC_NUMBER {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "无效的文件格式"));
    }

    // 读取metadata
    let mut meta_len_bytes = [0u8; 4];
    pak_file.read_exact(&mut meta_len_bytes)?;
    let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;
    if meta_len > 0 {
        // 跳过metadata内容
        pak_file.seek_relative(meta_len as i64)?;
        
        // 读取metadata结束标记
        let mut metadata_end = [0u8; 8];
        pak_file.read_exact(&mut metadata_end)?;
        if metadata_end != MAGIC_METADATA_END {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "无效的metadata结束标记"
            ));
        }
    }

    // 读取文件数量
    let mut count_bytes = [0u8; 4];
    pak_file.read_exact(&mut count_bytes)?;
    let count = u32::from_le_bytes(count_bytes);

    println!("文件列表 (完整扫描模式):");
    println!("----------------------------------------");
    
    let mut total_size = 0u64;
    for i in 0..count {
        // 读取文件路径
        let mut path_len_bytes = [0u8; 4];
        pak_file.read_exact(&mut path_len_bytes)?;
        let path_len = u32::from_le_bytes(path_len_bytes) as usize;

        let mut path_bytes = vec![0u8; path_len];
        pak_file.read_exact(&mut path_bytes)?;
        let path_str = String::from_utf8(path_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        // 读取文件大小
        let mut content_len_bytes = [0u8; 4];
        pak_file.read_exact(&mut content_len_bytes)?;
        let content_len = u32::from_le_bytes(content_len_bytes) as u64;

        println!("{:4}. {} ({} 字节)", i + 1, path_str, content_len);
        total_size += content_len;

        // 跳过文件
        pak_file.seek_relative(content_len as i64)?;
    }

    println!("----------------------------------------");
    println!("总大小: {} 字节", total_size);
    
    Ok(())
}