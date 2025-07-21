use crate::error::CompileError;
use std::{
    fs,
    path::{Path, PathBuf},
};

// NOTE: We can remove this if we can deterministically always knows where the risc0 artifacts
// will be.

/// RAII guard for backing up a file and ensuring its original state is restored
/// when the guard goes out of scope, or that a temporarily created file is deleted.
#[derive(Debug)]
pub struct FileRestorer {
    path: PathBuf,
    original_content: Option<Vec<u8>>,
    was_originally_present: bool,
}

impl FileRestorer {
    /// Creates a new FileRestorer for the given path.
    /// It reads and stores the original content if the file exists.
    pub fn new(path_to_manage: &Path) -> Result<Self, CompileError> {
        let was_originally_present = path_to_manage.exists();
        let original_content =
            if was_originally_present {
                if path_to_manage.is_dir() {
                    return Err(CompileError::InvalidMethodsPath(path_to_manage.into()));
                }
                Some(fs::read(path_to_manage).map_err(|e| {
                    CompileError::io(e, "FileRestorer: could not read original file")
                })?)
            } else {
                None
            };

        Ok(Self {
            path: path_to_manage.to_path_buf(),
            original_content,
            was_originally_present,
        })
    }
}

impl Drop for FileRestorer {
    fn drop(&mut self) {
        if let Some(content) = &self.original_content {
            // Original file existed, restore its content.
            if let Err(e) = fs::write(&self.path, content) {
                eprintln!(
                    "ERROR (FileRestorer): Failed to restore original content to file {}: {}. Manual restoration may be needed.",
                    self.path.display(),
                    e
                );
            }
        } else if self.was_originally_present {
            // This case (original file existed, but no content backed up) should ideally not be reached
            // if `new()` successfully read it or errored out. This implies an issue in `new()` logic or state.
            eprintln!(
                "ERROR (FileRestorer): Original file {} was present but no backup content was stored. Cannot restore properly.",
                self.path.display()
            );
        } else {
            // File was not originally present, so the file at `self.path` was created by the user of FileRestorer.
            // We should delete it.
            if self.path.exists() && !self.path.is_dir() {
                // Extra check for is_dir before remove_file
                if let Err(e) = fs::remove_file(&self.path) {
                    eprintln!(
                        "ERROR (FileRestorer): Failed to remove temporary file {}: {}. Manual removal may be needed.",
                        self.path.display(),
                        e
                    );
                }
            } else if self.path.exists() && self.path.is_dir() {
                eprintln!(
                    "ERROR (FileRestorer): Path {} was expected to be a file created by the operation, but it's a directory. Will not remove.",
                    self.path.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::{fs::File, io::Read};
    use tempfile::NamedTempFile;

    #[test]
    fn test_file_restorer_restores_existing_file() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let initial_content = b"initial content";
        fs::write(temp_file.path(), initial_content)?;

        let file_path = temp_file.path().to_path_buf();
        {
            let _restorer = FileRestorer::new(&file_path)?;
            // Modify the file while restorer is in scope
            fs::write(&file_path, b"modified content")?;
            let mut current_content = Vec::new();
            File::open(&file_path)?.read_to_end(&mut current_content)?;
            assert_eq!(current_content, b"modified content");
        } // _restorer goes out of scope here, Drop is called

        let mut final_content = Vec::new();
        File::open(&file_path)?.read_to_end(&mut final_content)?;
        assert_eq!(
            final_content, initial_content,
            "File content was not restored."
        );

        Ok(())
    }

    #[test]
    fn test_file_restorer_removes_created_file() -> Result<()> {
        let temp_file = NamedTempFile::new()?; // Creates a file
        let file_path = temp_file.path().to_path_buf();
        // Ensure it's deleted before the test so FileRestorer sees it as new
        drop(temp_file); // This deletes the file created by NamedTempFile
        assert!(
            !file_path.exists(),
            "Temp file should be deleted before FileRestorer test for creation."
        );

        {
            let _restorer = FileRestorer::new(&file_path)?;
            assert!(
                !file_path.exists(),
                "File should not exist yet if it was not originally present."
            );
            // Create the file while restorer is in scope
            fs::write(&file_path, b"newly created content")?;
            assert!(file_path.exists(), "File should exist after being written.");
        } // _restorer goes out of scope here, Drop is called

        assert!(
            !file_path.exists(),
            "Newly created file was not removed by FileRestorer."
        );
        Ok(())
    }

    #[test]
    fn test_file_restorer_handles_path_is_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = FileRestorer::new(temp_dir.path());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CompileError::InvalidMethodsPath(_)
        ));
    }
}
