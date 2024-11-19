use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use std::io::{self, Read, Write, Seek, SeekFrom};
use serde_json::Value;
use std::path::Path;
use std::fs::File;
use indicatif::{ProgressBar, ProgressStyle};

use crate::common::{FORMAT_VERSION, MAGIC_NUMBER, MAGIC_METADATA_END};

#[derive(Serialize, Deserialize, Debug)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
}

impl FileInfo {
    pub fn new(path: impl AsRef<Path>, size: u64) -> Self {
        Self {
            path: path.as_ref()
                .to_string_lossy()
                .replace('\\', "/")
                .to_string(),
            size
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct XpakMetadata {
    pub version: String,
    pub format_version: String,
    pub created_at: DateTime<Utc>,
    pub files_count: u32,
    pub total_size: u64,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub common: HashMap<String, Value>,
    pub files: Vec<FileInfo>,
}

impl Default for XpakMetadata {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            format_version: FORMAT_VERSION.to_string(),
            created_at: Utc::now(),
            files_count: 0,
            total_size: 0,
            description: None,
            common: HashMap::new(),
            files: Vec::new(),
        }
    }
}

impl XpakMetadata {
    pub fn new(files_count: u32, total_size: u64) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            format_version: FORMAT_VERSION.to_string(),
            created_at: Utc::now(),
            files_count,
            total_size,
            description: None,
            common: HashMap::new(),
            files: Vec::new(),
        }
    }

    pub fn merge_user_metadata(&mut self, user_meta: &str) -> Result<(), String> {
        println!("Raw metadata: {:?}", user_meta);
        
        match serde_json::from_str::<serde_json::Value>(user_meta) {
            Ok(user_value) => {
                if let serde_json::Value::Object(map) = user_value {
                    for (key, value) in map {
                        self.common.insert(key, value);
                    }
                    Ok(())
                } else {
                    Err("Metadata must be a JSON object".to_string())
                }
            }
            Err(e) => {
                Err(format!("Failed to parse JSON: {}. Input was: {:?}", e, user_meta))
            }
        }
    }
}

pub fn display_metadata(input: &str, show_files: bool) -> io::Result<()> {
    let mut file = File::open(input)?;

    // 读取并验证Magic Number
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if &magic != MAGIC_NUMBER {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid file format"));
    }

    // 读取metadata长度
    let mut meta_len_bytes = [0u8; 4];
    file.read_exact(&mut meta_len_bytes)?;
    let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;

    if meta_len == 0 {
        println!("No metadata found");
        return Ok(());
    }

    // 读取metadata内容
    let mut metadata_bytes = vec![0u8; meta_len];
    file.read_exact(&mut metadata_bytes)?;

    match serde_json::from_slice::<Value>(&metadata_bytes) {
        Ok(mut json) => {
            if !show_files {
                // 把files变为...
                json.get_mut("files").map(|v| *v = Value::String("...".to_string()));
            }
            print_json_tree("", &json);
        }
        Err(e) => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Failed to parse metadata: {}", e)));
        }
    }

    Ok(())
}

fn print_json_tree(prefix: &str, value: &Value) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));

            // 去除没有值的键
            entries.retain(|(_, val)| !val.is_null());
            
            for (i, (key, val)) in entries.iter().enumerate() {
                let is_last = i == entries.len() - 1;
                let node = if is_last { "└─" } else { "├─" };
                let next_prefix = format!("{}{}   ", prefix, if is_last { " " } else { "│" });
                
                match val {
                    Value::Object(_) | Value::Array(_) => {
                        println!("{}{} {}:", prefix, node, key);
                        print_json_tree(&next_prefix, val);
                    }
                    _ => {
                        println!("{}{} {}: {}", prefix, node, key, val);
                    }
                }
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                let is_last = i == arr.len() - 1;
                let node = if is_last { "└─" } else { "├─" };
                let next_prefix = format!("{}{}   ", prefix, if is_last { " " } else { "│" });
                
                match val {
                    Value::Object(_) | Value::Array(_) => {
                        println!("{}{}", prefix, node);
                        print_json_tree(&next_prefix, val);
                    }
                    _ => {
                        println!("{}{} {}", prefix, node, val);
                    }
                }
            }
        }
        _ => println!("{}{}", prefix, value),
    }
} 

pub fn update_metadata(input: &str, description: Option<&str>, metadata: Option<&str>, all: bool) -> io::Result<()> {
    let mut file = File::open(input)?;
    
    // 读取并验证Magic Number
    println!("读取并验证Magic Number");
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if &magic != MAGIC_NUMBER {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "无效的文件格式"));
    }

    // 读取metadata长度
    println!("读取metadata长度");
    let mut meta_len_bytes = [0u8; 4];
    file.read_exact(&mut meta_len_bytes)?;
    let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;

    let mut metadata_bytes = vec![0u8; meta_len];
    file.read_exact(&mut metadata_bytes)?;
    
    // 读取并验证metadata结束标志
    println!("读取并验证metadata结束标志");
    let mut end_magic = [0u8; 8];
    file.read_exact(&mut end_magic)?;
    if end_magic != MAGIC_METADATA_END {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "无效的metadata结束标志"));
    }

    let mut xpak_meta: XpakMetadata = if all {
        // 如果是全部重新生成，创建新的metadata
        let data_offset = 8 + meta_len as u64 + 8; // magic(4) + len(4) + metadata + end_magic(8)
        file.seek(SeekFrom::Start(data_offset))?;
        
        // 计算文件总大小
        println!("计算文件总大小");
        let mut total_size = 0u64;
        let mut files = Vec::new();
        
        // 读取文件头部信息
        println!("读取文件头部信息");
        loop {
            let mut name_len_bytes = [0u8; 4];
            if file.read_exact(&mut name_len_bytes).is_err() {
                break;
            }
            let name_len = u32::from_le_bytes(name_len_bytes) as usize;
            
            let mut name_bytes = vec![0u8; name_len];
            file.read_exact(&mut name_bytes)?;
            let name = String::from_utf8_lossy(&name_bytes).to_string();
            
            let mut size_bytes = [0u8; 8];
            file.read_exact(&mut size_bytes)?;
            let size = u64::from_le_bytes(size_bytes);
            
            total_size += size;
            files.push(FileInfo::new(name, size));
            
            // 跳过文件内容
            file.seek(SeekFrom::Current(size as i64))?;
        }
        
        let mut new_meta = XpakMetadata::new(files.len() as u32, total_size);
        new_meta.files = files;
        new_meta
    } else {
        serde_json::from_slice(&metadata_bytes)?
    };

    // 更新描述信息
    println!("更新描述信息");
    if let Some(desc) = description {
        xpak_meta.description = Some(desc.to_string());
    }

    // 更新用户自定义metadata
    println!("更新用户自定义metadata");
    if let Some(meta_str) = metadata {
        xpak_meta.merge_user_metadata(meta_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    }

    // 将更新后的metadata写回文件
    println!("将更新后的metadata写回文件");
    let new_metadata = serde_json::to_vec(&xpak_meta)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("序列化metadata失败: {}", e)))?;

    // 创建临时文件
    println!("创建临时文件");
    let temp_path = format!("{}.tmp", input);
    let mut temp_file = File::create(&temp_path)?;

    // 写入Magic Number
    println!("写入Magic Number");
    temp_file.write_all(MAGIC_NUMBER)?;

    // 写入新的metadata长度
    println!("写入新的metadata长度");
    temp_file.write_all(&(new_metadata.len() as u32).to_le_bytes())?;

    // 写入新的metadata
    println!("写入新的metadata");
    temp_file.write_all(&new_metadata)?;
    
    // 写入metadata结束标志
    println!("写入metadata结束标志");
    temp_file.write_all(&MAGIC_METADATA_END)?;

    // 复制剩余的文件数据（从数据区域开始）
    println!("复制剩余的文件数据（从数据区域开始）");
    file.seek(SeekFrom::Start(8 + meta_len as u64 + 8))?; // 跳过原始metadata部分和结束标志
    
    // 获取剩余需要复制的数据大小
    let remaining_size = file.metadata()?.len() - file.stream_position()?;
    
    // 创建进度条
    let pb = ProgressBar::new(remaining_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("#>-"));
    
    // 使用自定义的 io::copy 来更新进度
    let mut buffer = [0; 8192];
    let mut copied = 0u64;
    loop {
        let n = match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        temp_file.write_all(&buffer[..n])?;
        copied += n as u64;
        pb.set_position(copied);
    }
    
    pb.finish_with_message("复制完成");

    // 关闭文件
    drop(file);
    drop(temp_file);

    // 替换原文件
    println!("替换原文件");
    std::fs::rename(temp_path, input)?;

    Ok(())
} 