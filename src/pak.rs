use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::io::{self, Read, Write, BufReader, BufWriter};
use std::sync::atomic::{AtomicBool, Ordering};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use walkdir::WalkDir;
use std::sync::Arc;
use std::fs::File;
use chrono::Utc;

use crate::common::{BUFFER_SIZE, FORMAT_VERSION, MAGIC_METADATA_END, MAGIC_NUMBER};
use crate::metadata::{XpakMetadata, FileInfo};

pub fn pack_files(
    input: &str, 
    output: &str, 
    flat: bool, 
    description: Option<&str>,
    metadata: Option<&str>,
    running: Arc<AtomicBool>
) -> io::Result<()> {
    let input_path = Path::new(input);
    
    if !input_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Input path '{}' does not exist", input)
        ));
    }

    // 收集文件信息
    let files: Vec<_> = WalkDir::new(input)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();
    
    // 计算总大小
    let total_size: u64 = files.iter()
        .filter_map(|entry| entry.metadata().ok())
        .map(|meta| meta.len())
        .sum();

    // 创建基础metadata
    let mut xpak_meta = XpakMetadata::new(files.len() as u32, total_size);

    // 如果有提供的描述，设置描述
    if let Some(desc) = description {
        xpak_meta.description = Some(desc.to_string());
    }

    // 如果有提供的metadata，验证并合并它
    if let Some(meta) = metadata {
        let user_meta = if meta.len() % 4 == 0 && meta.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=') {
            // 尝试base64解码
            match STANDARD.decode(meta.as_bytes()) {
                Ok(decoded) => match String::from_utf8(decoded) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Invalid UTF-8 in decoded base64: {}", e)
                        ));
                    }
                },
                Err(e) => {
                    // 如果不是有效的 base64，就使用原始字符串
                    println!("Invalid base64 metadata: {}", e);
                    println!("Using original metadata: {}", meta);
                    meta.to_string()
                }
            }
        } else {
            meta.to_string()
        };

        // 验证metadata格式
        if let Err(e) = xpak_meta.merge_user_metadata(&user_meta) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Metadata error: {}", e)
            ));
        }
    }

    // 开始写入文件
    let mut pak_file = BufWriter::with_capacity(BUFFER_SIZE, File::create(output)?);

    // 写入Magic Number
    pak_file.write_all(MAGIC_NUMBER)?;

    // 序列化 metadata 处理并写入
    let mut metadata_content = XpakMetadata {
        version: env!("CARGO_PKG_VERSION").to_string(),
        format_version: FORMAT_VERSION.to_string(),
        created_at: Utc::now(),
        files_count: files.len() as u32,
        total_size,
        description: metadata.map(|s| s.to_string()),
        common: HashMap::new(),
        files: files.iter().map(|entry| {
            let path = entry.path();
            let relative_path = path.strip_prefix(input_path).unwrap();
            let file_path = if flat {
                PathBuf::from(path.file_name().unwrap())
            } else {
                relative_path.to_path_buf()
            };
            
            FileInfo::new(file_path, entry.metadata().unwrap().len())
        }).collect(),
    };

    // 合并metadata
    if let Some(meta_str) = metadata {
        if let Err(e) = metadata_content.merge_user_metadata(meta_str) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Metadata error: {}", e)
            ));
        }
    }

    let metadata_bytes = serde_json::to_vec(&metadata_content)?;
    pak_file.write_all(&(metadata_bytes.len() as u32).to_le_bytes())?;
    pak_file.write_all(&metadata_bytes)?;

    // 添加metadata结束标记（8字节）
    pak_file.write_all(&MAGIC_METADATA_END)?;

    // 写入文件数量
    pak_file.write_all(&(files.len() as u32).to_le_bytes())?;

    // 预分配缓冲区
    let mut buffer = vec![0u8; BUFFER_SIZE];

    // 进度条
    let progress = ProgressBar::new(total_size);
    progress.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("#>-"));
    
    // 写入文件内容
    for entry in files {
        if !running.load(Ordering::SeqCst) {
            drop(pak_file);
            if Path::new(output).exists() {
                std::fs::remove_file(output)?;
            }
            return Err(io::Error::new(io::ErrorKind::Interrupted, "操作被用户取消"));
        }

        let path = entry.path();
        let relative_path = path.strip_prefix(input_path).unwrap();
        let file_path = if flat {
            PathBuf::from(path.file_name().unwrap())
        } else {
            relative_path.to_path_buf()
        };

        // 写入文件路径
        let path_str = file_path.to_string_lossy();
        pak_file.write_all(&(path_str.len() as u32).to_le_bytes())?;
        pak_file.write_all(path_str.as_bytes())?;

        // 优化文件内容写入
        let mut file = BufReader::with_capacity(BUFFER_SIZE, File::open(path)?);
        let file_size = file.get_ref().metadata()?.len() as usize;

        pak_file.write_all(&(file_size as u32).to_le_bytes())?;

        if file_size >= BUFFER_SIZE {
            // 大文件使用 io::copy
            io::copy(&mut file, &mut pak_file)?;
        } else {
            // 小文件使用缓冲区
            let mut remaining = file_size;
            while remaining > 0 {
                let to_read = remaining.min(BUFFER_SIZE);
                let buf = &mut buffer[..to_read];
                file.read_exact(buf)?;
                pak_file.write_all(buf)?;
                remaining -= to_read;
            }
        }
        
        progress.inc(file_size as u64);
    }

    // 确保所有数据都写入磁盘
    pak_file.flush()?;
    progress.finish();

    Ok(())
}
