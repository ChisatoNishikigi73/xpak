pub const MAGIC_NUMBER: &[u8] = b"XPAK";
pub const BUFFER_SIZE: usize = 65536;  // 64KB 缓冲区

pub const FORMAT_VERSION: &str = "1.0";

pub const DIRECT_COPY_THRESHOLD: usize = 1024 * 1024;  // 1MB，大文件直接复制阈值