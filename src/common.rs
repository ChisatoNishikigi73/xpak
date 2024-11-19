pub const MAGIC_NUMBER: &[u8] = b"XPAK";
pub const MAGIC_METADATA_END: [u8; 8] = [0x4d, 0x45, 0x54, 0x41, 0x45, 0x4e, 0x44, 0x5f]; // METAEND_

// pub const MAGIC_FILE_START: [u8; 8] = [0x5f, 0x46, 0x53, 0x54, 0x41, 0x52, 0x54, 0x5f]; // _FSTART_
// pub const MAGIC_FILE_END: [u8; 8] = [0x5f, 0x46, 0x49, 0x4c, 0x45, 0x4e, 0x44, 0x5f]; // _FILEND_
// pub const MAGIC_FILE_PATH: [u8; 8] = [0x5f, 0x50, 0x41, 0x54, 0x48, 0x5f, 0x5f, 0x5f]; // _PATH__

pub const BUFFER_SIZE: usize = 65536;  // 64KB 缓冲区

pub const FORMAT_VERSION: &str = "1.3";

// pub const DIRECT_COPY_THRESHOLD: usize = 1024 * 1024;  // 1MB，大文件直接复制阈值

#[allow(dead_code)]
pub const KB: usize = 1024;  // 1KB
#[allow(dead_code)]
pub const MB: usize = 1024 * 1024;  // 1MB
#[allow(dead_code)]
pub const GB: usize = 1024 * 1024 * 1024;  // 1GB