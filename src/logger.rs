use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
const MAX_COMPRESSED_LOGS: usize = 5;

static SERVER_LOGGER: OnceLock<Mutex<RotatingLogger>> = OnceLock::new();
static REPEATED_MESSAGES: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();

pub fn init_server_log(path: impl AsRef<Path>) -> Result<(), String> {
    let path = path.as_ref().to_path_buf();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    SERVER_LOGGER
        .set(Mutex::new(RotatingLogger { path }))
        .map_err(|_| "서버 로그가 이미 초기화되었습니다.".to_string())
}

pub fn write_line(message: &str, is_error: bool) {
    if is_error {
        eprintln!("{message}");
    } else {
        println!("{message}");
    }
    let Some(logger) = SERVER_LOGGER.get() else {
        return;
    };
    if let Ok(mut logger) = logger.lock()
        && let Err(error) = logger.append(message)
    {
        eprintln!("서버 로그 기록 실패: {error}");
    }
}

/// 같은 네트워크 오류는 최초와 10·100·1000회 누적 시점에만 새 줄로 출력합니다.
/// carriage return으로 기존 줄을 덮어쓰지 않아 systemd와 비동기 로그가 섞이지 않습니다.
pub fn write_repeated_error(message: &str) {
    write_repeated(message, true);
}

/// 같은 정상 이벤트도 최초와 누적 요약 시점에만 새 줄로 출력합니다.
pub fn write_repeated_info(message: &str) {
    write_repeated(message, false);
}

fn write_repeated(message: &str, is_error: bool) {
    let count = {
        let counters = REPEATED_MESSAGES.get_or_init(|| Mutex::new(HashMap::new()));
        let Ok(mut counters) = counters.lock() else {
            write_line(message, is_error);
            return;
        };
        let count = counters.entry(message.to_owned()).or_insert(0);
        *count = count.saturating_add(1);
        *count
    };
    if count == 1 || is_decimal_checkpoint(count) {
        let suffix = if count == 1 {
            "1회, 이후 동일 메시지 집계".to_string()
        } else {
            format!("{count}회 누적")
        };
        let line = format!("{message} ({suffix})");
        if is_error {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
        if let Some(logger) = SERVER_LOGGER.get()
            && let Ok(mut logger) = logger.lock()
        {
            let _ = logger.append(&line);
        }
    }
}

fn is_decimal_checkpoint(count: usize) -> bool {
    count >= 10 && count.to_string().bytes().skip(1).all(|digit| digit == b'0')
}

struct RotatingLogger {
    path: PathBuf,
}

impl RotatingLogger {
    fn append(&mut self, message: &str) -> Result<(), String> {
        let additional = message.len() as u64 + 1;
        let current = fs::metadata(&self.path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if current > 0 && current.saturating_add(additional) > MAX_LOG_BYTES {
            self.rotate()?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| error.to_string())?;
        writeln!(file, "{message}").map_err(|error| error.to_string())
    }

    fn rotate(&self) -> Result<(), String> {
        let oldest = rotated_path(&self.path, MAX_COMPRESSED_LOGS);
        if oldest.exists() {
            fs::remove_file(oldest).map_err(|error| error.to_string())?;
        }
        for index in (1..MAX_COMPRESSED_LOGS).rev() {
            let source = rotated_path(&self.path, index);
            if source.exists() {
                fs::rename(source, rotated_path(&self.path, index + 1))
                    .map_err(|error| error.to_string())?;
            }
        }
        let input = File::open(&self.path).map_err(|error| error.to_string())?;
        let output =
            File::create(rotated_path(&self.path, 1)).map_err(|error| error.to_string())?;
        let mut encoder =
            zstd::stream::write::Encoder::new(output, 3).map_err(|error| error.to_string())?;
        std::io::copy(&mut BufReader::new(input), &mut encoder)
            .map_err(|error| error.to_string())?;
        let output = encoder.finish().map_err(|error| error.to_string())?;
        output.sync_all().map_err(|error| error.to_string())?;
        File::create(&self.path).map_err(|error| error.to_string())?;
        Ok(())
    }
}

fn rotated_path(path: &Path, index: usize) -> PathBuf {
    PathBuf::from(format!("{}.{}.zst", path.display(), index))
}

#[macro_export]
macro_rules! log_info {
    ($($argument:tt)*) => {
        $crate::logger::write_line(&format!($($argument)*), false)
    };
}

#[macro_export]
macro_rules! log_error {
    ($($argument:tt)*) => {
        $crate::logger::write_line(&format!($($argument)*), true)
    };
}

#[cfg(test)]
mod tests {
    use super::is_decimal_checkpoint;

    #[test]
    fn repeated_log_checkpoints_are_sparse() {
        assert!(!is_decimal_checkpoint(2));
        assert!(is_decimal_checkpoint(10));
        assert!(is_decimal_checkpoint(100));
        assert!(!is_decimal_checkpoint(101));
    }
}
