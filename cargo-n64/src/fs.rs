use fatfs::{self, FileSystem, FormatVolumeOptions, FsOptions};
use std::fs::{self, metadata, read_dir, DirEntry};
use std::io::{self, Cursor, Write};
use std::path::{Path, StripPrefixError};

use failure::Fail;

#[derive(Debug, Fail)]
pub enum FSError {
    #[fail(display = "IO Error")]
    IOError(#[cause] io::Error),

    #[fail(display = "Error strippping path prefix")]
    StripPrefixError(#[cause] StripPrefixError),

    #[fail(display = "Missing file name")]
    MissingFileName,
}

impl From<io::Error> for FSError {
    fn from(e: io::Error) -> Self {
        FSError::IOError(e)
    }
}

impl From<StripPrefixError> for FSError {
    fn from(e: StripPrefixError) -> Self {
        FSError::StripPrefixError(e)
    }
}

fn traverse<T>(
    path: &impl AsRef<Path>,
    mut acc: T,
    cb: &impl Fn(T, &DirEntry) -> Result<T, FSError>,
) -> Result<T, FSError> {
    for entry in read_dir(path)? {
        let entry = entry?;

        // Accumulate
        acc = cb(acc, &entry)?;

        // Recursively call into directories and accumulate
        let path = entry.path();
        if path.is_dir() {
            acc = traverse(&path, acc, cb)?;
        }
    }
    Ok(acc)
}

crate fn create_filesystem(fs_path: impl AsRef<Path>) -> Result<Vec<u8>, FSError> {
    // Make sure the path is normalized to absolute.
    let fs_path = fs_path.as_ref().canonicalize()?;

    // Minimum number of bytes reserved for FAT
    // FIXME: Is this enough in general?
    const RESERVED_BYTES: usize = 128 * 1024;

    // Compute the required volume size
    // WARNING: This is not atomic! Any changes to the file system after this
    // computation starts will surely break things later!
    let size = traverse(&fs_path, RESERVED_BYTES, &|mut size, entry| {
        let stat = metadata(&entry.path())?;
        if stat.is_file() {
            size += (stat.len() as usize + 511) & !512;
        }
        Ok(size)
    })?;

    // Create a new in-memory volume
    let mut stream = Cursor::new(vec![0; size]);
    let opts = {
        let opts = FormatVolumeOptions::new();
        opts.volume_label(*b"TECHNEKDISK")
    };
    fatfs::format_volume(&mut stream, opts)?;

    // This scope allows us to consume `stream` without explicitly dropping `disk`
    {
        let disk = FileSystem::new(&mut stream, FsOptions::new())?;
        let root_dir = disk.root_dir();

        // Traverse the directory again, this time copying file contents and creating directories.
        traverse(&fs_path, (), &|(), entry| {
            let path = entry.path();
            let name = &path.strip_prefix(&fs_path)?.to_string_lossy();

            if entry.file_type()?.is_dir() {
                root_dir.create_dir(name)?;
            } else {
                let buffer = fs::read(&path)?;
                let mut dest = root_dir.create_file(name)?;
                dest.write_all(&buffer)?;
            }

            Ok(())
        })?;
    }

    Ok(stream.into_inner())
}
