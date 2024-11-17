use crate::common::{BUFFER_SIZE, DIRECT_COPY_THRESHOLD, MAGIC_NUMBER};
use crate::metadata;

use std::io::{self, Read, Write, BufReader, BufWriter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs::{self, File};
use std::path::Path;
use std::sync::Arc;

pub fn unpack_files(
    input: &str, 
    output: &str, 
    selected_files: Option<&[String]>,
    running: Arc<AtomicBool>
) -> io::Result<()> {
    let output_path = Path::new(output);
    fs::create_dir_all(output_path)?;

    let mut pak_file = BufReader::new(File::open(input)?);

    // 读取并验证Magic Number
    let mut magic = [0u8; 4];
    pak_file.read_exact(&mut magic)?;
    if &magic != MAGIC_NUMBER {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid file format"));
    }

    // 读取metadata
    let mut meta_len_bytes = [0u8; 4];
    pak_file.read_exact(&mut meta_len_bytes)?;
    let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;

    if meta_len > 0 {
        metadata::display_metadata(input)?;
        
        // 跳过 metadata 内容
        let mut metadata_bytes = vec![0u8; meta_len];
        pak_file.read_exact(&mut metadata_bytes)?;
    }

    // 读取文件数量
    let mut count_bytes = [0u8; 4];
    pak_file.read_exact(&mut count_bytes)?;
    let count = u32::from_le_bytes(count_bytes);

    // 预分配缓冲区
    let mut buffer = vec![0u8; BUFFER_SIZE];

    let mut files_unpacked = 0;

    for _ in 0..count {
        if !running.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "操作被用户取消"
            ));
        }

        // 读取文件路径
        let mut path_len_bytes = [0u8; 4];
        pak_file.read_exact(&mut path_len_bytes)?;
        let path_len = u32::from_le_bytes(path_len_bytes) as usize;

        let mut path_bytes = vec![0u8; path_len];
        pak_file.read_exact(&mut path_bytes)?;
        let path_str = String::from_utf8(path_bytes).unwrap();
        
        let mut content_len_bytes = [0u8; 4];
        pak_file.read_exact(&mut content_len_bytes)?;
        let content_len = u32::from_le_bytes(content_len_bytes) as usize;

        // 检查是否需要解包此文件
        let should_unpack = selected_files.map_or(true, |files| {
            files.iter().any(|f| path_str == *f)
        });

        if should_unpack {
            // 创建输出文件
            let file_path = output_path.join(&path_str);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let file = File::create(&file_path)?;
            let mut writer = BufWriter::new(file);

            if content_len >= DIRECT_COPY_THRESHOLD {
                // 大文件使用直接拷贝，但不移动所有权
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
            drop(writer);

            println!("已解包: {}", path_str);
            files_unpacked += 1;
        } else {
            // 跳过不需要的文件
            pak_file.seek_relative(content_len as i64)?;
        }
    }

    println!("共解包 {} 个文件", files_unpacked);
    Ok(())
}

pub fn list_files(input: &str) -> io::Result<()> {
    let file = File::open(input)?;
    let mut pak_file = BufReader::with_capacity(BUFFER_SIZE, file);

    // 验证Magic Number
    let mut magic = [0u8; 4];
    pak_file.read_exact(&mut magic)?;
    if &magic != MAGIC_NUMBER {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "无效的文件格式"));
    }

    // 跳过metadata部分
    let mut meta_len_bytes = [0u8; 4];
    pak_file.read_exact(&mut meta_len_bytes)?;
    let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;
    if meta_len > 0 {
        let mut metadata_bytes = vec![0u8; meta_len];
        pak_file.read_exact(&mut metadata_bytes)?;
    }

    // 读取文件数量
    let mut count_bytes = [0u8; 4];
    pak_file.read_exact(&mut count_bytes)?;
    let count = u32::from_le_bytes(count_bytes);

    println!("文件列表 (共 {} 个文件):", count);
    println!("----------------------------------------");

    let mut total_size: u64 = 0;

    for i in 0..count {
        // 读取文件路径
        let mut path_len_bytes = [0u8; 4];
        pak_file.read_exact(&mut path_len_bytes)?;
        let path_len = u32::from_le_bytes(path_len_bytes) as usize;

        let mut path_bytes = vec![0u8; path_len];
        pak_file.read_exact(&mut path_bytes)?;
        let path_str = String::from_utf8(path_bytes).unwrap();
        
        // 读取文件大小
        let mut content_len_bytes = [0u8; 4];
        pak_file.read_exact(&mut content_len_bytes)?;
        let content_len = u32::from_le_bytes(content_len_bytes) as usize;
        
        // 跳过文件内容
        pak_file.seek_relative(content_len as i64)?;
        
        total_size += content_len as u64;
        println!("{:4}. {} ({} 字节)", i + 1, path_str, content_len);
    }

    println!("----------------------------------------");
    println!("总大小: {} 字节", total_size);

    Ok(())
}