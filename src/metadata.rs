use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use std::fs::File;
use std::io::{self, Read};
use serde_json::Value;

use crate::common::FORMAT_VERSION;

#[derive(Serialize, Deserialize, Debug)]
pub struct XpakMetadata {
    pub version: String,
    pub format_version: String,
    pub created_at: DateTime<Utc>,
    pub files_count: u32,
    pub total_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub common: HashMap<String, Value>,
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

pub fn display_metadata(input: &str) -> io::Result<()> {
    let mut file = File::open(input)?;

    // 读取并验证魔数
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if &magic != b"XPAK" {
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
        Ok(json) => {
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