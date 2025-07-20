use std::collections::HashSet;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("File too large: {size} bytes (max: {max_size} bytes)")]
    FileTooLarge { size: u64, max_size: u64 },
    
    #[error("Invalid file type: {content_type} (allowed: {allowed:?})")]
    InvalidFileType { content_type: String, allowed: Vec<String> },
    
    #[error("Filename too long: {length} characters (max: {max_length})")]
    FilenameTooLong { length: usize, max_length: usize },
    
    #[error("Invalid filename: {filename}")]
    InvalidFilename { filename: String },
    
    #[error("Empty file not allowed")]
    EmptyFile,
    
    #[error("Suspicious file content detected")]
    SuspiciousContent,
}

#[derive(Debug, Clone)]
pub struct FileValidationConfig {
    pub max_file_size: u64,
    pub allowed_content_types: HashSet<String>,
    pub max_filename_length: usize,
    pub check_magic_bytes: bool,
}

impl Default for FileValidationConfig {
    fn default() -> Self {
        let mut allowed_types = HashSet::new();
        
        allowed_types.insert("image/jpeg".to_string());
        allowed_types.insert("image/png".to_string());
        allowed_types.insert("image/gif".to_string());
        allowed_types.insert("image/webp".to_string());
        allowed_types.insert("image/svg+xml".to_string());
        
        allowed_types.insert("application/pdf".to_string());
        allowed_types.insert("text/plain".to_string());
        allowed_types.insert("text/csv".to_string());
        allowed_types.insert("application/json".to_string());
        allowed_types.insert("application/xml".to_string());
        
        allowed_types.insert("application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string());
        allowed_types.insert("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string());
        allowed_types.insert("application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string());
        
        allowed_types.insert("application/zip".to_string());
        allowed_types.insert("application/x-tar".to_string());
        allowed_types.insert("application/gzip".to_string());
        
        Self {
            max_file_size: 10 * 1024 * 1024,
            allowed_content_types: allowed_types,
            max_filename_length: 255,
            check_magic_bytes: true,
        }
    }
}

#[derive(Clone)]
pub struct FileValidator {
    config: FileValidationConfig,
}

impl FileValidator {
    pub fn new(config: FileValidationConfig) -> Self {
        Self { config }
    }
    
    pub fn with_default_config() -> Self {
        Self::new(FileValidationConfig::default())
    }
    
    pub fn validate_upload(&self, filename: &str, content_type: &str, data: &[u8]) -> Result<(), ValidationError> {
        if data.is_empty() {
            return Err(ValidationError::EmptyFile);
        }
        
        if data.len() as u64 > self.config.max_file_size {
            return Err(ValidationError::FileTooLarge {
                size: data.len() as u64,
                max_size: self.config.max_file_size,
            });
        }
        
        self.validate_filename(filename)?;
        
        self.validate_content_type(content_type)?;
        
        if self.config.check_magic_bytes {
            self.validate_magic_bytes(content_type, data)?;
        }
        
        self.check_suspicious_content(data)?;
        
        Ok(())
    }
    
    fn validate_filename(&self, filename: &str) -> Result<(), ValidationError> {
        if filename.len() > self.config.max_filename_length {
            return Err(ValidationError::FilenameTooLong {
                length: filename.len(),
                max_length: self.config.max_filename_length,
            });
        }
        
        if filename.contains('\0') || filename.contains('/') || filename.contains('\\') {
            return Err(ValidationError::InvalidFilename {
                filename: filename.to_string(),
            });
        }
        
        let reserved_names = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];
        
        let name_without_ext = filename.split('.').next().unwrap_or(filename).to_uppercase();
        if reserved_names.contains(&name_without_ext.as_str()) {
            return Err(ValidationError::InvalidFilename {
                filename: filename.to_string(),
            });
        }
        
        Ok(())
    }
    
    fn validate_content_type(&self, content_type: &str) -> Result<(), ValidationError> {
        if !self.config.allowed_content_types.contains(content_type) {
            return Err(ValidationError::InvalidFileType {
                content_type: content_type.to_string(),
                allowed: self.config.allowed_content_types.iter().cloned().collect(),
            });
        }
        Ok(())
    }
    
    fn validate_magic_bytes(&self, content_type: &str, data: &[u8]) -> Result<(), ValidationError> {
        if data.len() < 4 {
            return Ok(());
        }
        
        let magic_matches = match content_type {
            "image/jpeg" => data.starts_with(&[0xFF, 0xD8, 0xFF]),
            "image/png" => data.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
            "image/gif" => data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a"),
            "application/pdf" => data.starts_with(b"%PDF"),
            "application/zip" => data.starts_with(&[0x50, 0x4B, 0x03, 0x04]) || data.starts_with(&[0x50, 0x4B, 0x05, 0x06]) || data.starts_with(&[0x50, 0x4B, 0x07, 0x08]),
            _ => true,
        };
        
        if !magic_matches {
            return Err(ValidationError::SuspiciousContent);
        }
        
        Ok(())
    }
    
    fn check_suspicious_content(&self, data: &[u8]) -> Result<(), ValidationError> {
        let suspicious_patterns: &[&[u8]] = &[
            b"<script",
            b"javascript:",
            b"vbscript:",
            b"onload=",
            b"onerror=",
            b"MZ",
            b"\x7fELF",
        ];
        
        for pattern in suspicious_patterns {
            if data.windows(pattern.len()).any(|window| {
                window.to_ascii_lowercase() == pattern.to_ascii_lowercase()
            }) {
                return Err(ValidationError::SuspiciousContent);
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_filename() {
        let validator = FileValidator::with_default_config();
        
        assert!(validator.validate_filename("test.txt").is_ok());
        assert!(validator.validate_filename("image.png").is_ok());
        assert!(validator.validate_filename("document with spaces.pdf").is_ok());
        
        assert!(validator.validate_filename("test/file.txt").is_err());
        assert!(validator.validate_filename("test\\file.txt").is_err());
        assert!(validator.validate_filename("test\0file.txt").is_err());
        assert!(validator.validate_filename("CON.txt").is_err());
        
        let long_name = "a".repeat(300);
        assert!(validator.validate_filename(&long_name).is_err());
    }
    
    #[test]
    fn test_validate_content_type() {
        let validator = FileValidator::with_default_config();
        
        assert!(validator.validate_content_type("image/jpeg").is_ok());
        assert!(validator.validate_content_type("application/pdf").is_ok());
        
        assert!(validator.validate_content_type("application/x-executable").is_err());
    }
    
    #[test]
    fn test_validate_magic_bytes() {
        let validator = FileValidator::with_default_config();
        
        let jpeg_data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        assert!(validator.validate_magic_bytes("image/jpeg", &jpeg_data).is_ok());
        
        let fake_jpeg = vec![0x00, 0x00, 0x00, 0x00];
        assert!(validator.validate_magic_bytes("image/jpeg", &fake_jpeg).is_err());
        
        let png_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert!(validator.validate_magic_bytes("image/png", &png_data).is_ok());
    }
    
    #[test]
    fn test_check_suspicious_content() {
        let validator = FileValidator::with_default_config();
        
        let safe_data = b"This is a normal text file.";
        assert!(validator.check_suspicious_content(safe_data).is_ok());
        
        let script_data = b"<script>alert('xss')</script>";
        assert!(validator.check_suspicious_content(script_data).is_err());
        
        let js_data = b"javascript:alert('xss')";
        assert!(validator.check_suspicious_content(js_data).is_err());
    }
}