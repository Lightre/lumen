use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub enum Direction {
    Previous,
    Next,
}

#[derive(Clone)]
pub struct Folder {
    files: Arc<[PathBuf]>,
    index: usize,
}

impl Folder {
    pub fn scan(current: &Path) -> Result<Self, String> {
        let current = current
            .canonicalize()
            .map_err(|_| "The image folder is unavailable.")?;
        let parent = current.parent().ok_or("The image folder is unavailable.")?;
        let entries =
            std::fs::read_dir(parent).map_err(|_| "The image folder could not be read.")?;
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| (supported(path) || *path == current) && path.is_file())
            .collect();
        if !files.contains(&current) {
            files.push(current.clone());
        }
        files.sort_by(|left, right| {
            natord::compare_ignore_case(
                &left.file_name().unwrap_or_default().to_string_lossy(),
                &right.file_name().unwrap_or_default().to_string_lossy(),
            )
            .then_with(|| left.cmp(right))
        });
        let index = files.iter().position(|path| *path == current).unwrap_or(0);
        Ok(Self {
            files: files.into(),
            index,
        })
    }

    pub fn single(path: PathBuf) -> Self {
        Self {
            files: vec![path].into(),
            index: 0,
        }
    }

    pub fn path(&self) -> &Path {
        &self.files[self.index]
    }

    pub fn position(&self) -> (usize, usize) {
        (self.index + 1, self.files.len())
    }

    pub fn neighbor(&self, direction: Direction) -> Option<Self> {
        let index = match direction {
            Direction::Previous => self.index.checked_sub(1)?,
            Direction::Next => self.index.checked_add(1)?,
        };
        (index < self.files.len()).then(|| Self {
            files: self.files.clone(),
            index,
        })
    }
}

fn supported(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["jpg", "jpeg", "png", "webp", "gif"]
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_only_supported_files_and_sorts_naturally() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["photo10.png", "Photo2.JPG", "photo1.webp", "notes.txt"] {
            std::fs::write(directory.path().join(name), []).unwrap();
        }
        std::fs::create_dir(directory.path().join("folder.png")).unwrap();
        let folder = Folder::scan(&directory.path().join("Photo2.JPG")).unwrap();
        assert_eq!(folder.position(), (2, 3));
        assert_eq!(
            folder
                .neighbor(Direction::Previous)
                .unwrap()
                .path()
                .file_name()
                .unwrap(),
            "photo1.webp"
        );
        assert_eq!(
            folder
                .neighbor(Direction::Next)
                .unwrap()
                .path()
                .file_name()
                .unwrap(),
            "photo10.png"
        );
    }

    #[test]
    fn stops_at_boundaries() {
        let folder = Folder::single("only.png".into());
        assert!(folder.neighbor(Direction::Previous).is_none());
        assert!(folder.neighbor(Direction::Next).is_none());
    }

    #[test]
    fn sorts_mixed_case_gif_names_naturally() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["FRAME10.GiF", "frame2.gif", "Frame1.GIF"] {
            std::fs::write(directory.path().join(name), []).unwrap();
        }
        let folder = Folder::scan(&directory.path().join("frame2.gif")).unwrap();
        assert_eq!(folder.position(), (2, 3));
        assert_eq!(
            folder
                .neighbor(Direction::Previous)
                .unwrap()
                .path()
                .file_name()
                .unwrap(),
            "Frame1.GIF"
        );
        assert_eq!(
            folder
                .neighbor(Direction::Next)
                .unwrap()
                .path()
                .file_name()
                .unwrap(),
            "FRAME10.GiF"
        );
    }

    #[test]
    fn retains_explicitly_opened_file_with_unrecognized_extension() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("photo.data");
        std::fs::write(&path, []).unwrap();
        let folder = Folder::scan(&path).unwrap();
        assert_eq!(folder.position(), (1, 1));
        assert_eq!(folder.path(), path.canonicalize().unwrap());
    }
}
