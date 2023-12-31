//! This crate provides some utility functions for files generated by Rockwell Software's `FactoryTalk View Studio`
//!
//! Provides the ability to check the version the file was generated as,
//! Provides the ability to check if the file is locked, or if an MER is marked as never restore
//! If the file is locked or marked as never restore this can be cleared so the file can be opened

#![deny(clippy::all)]
#![deny(clippy::pedantic)]

use std::io::{Read, Write};
use std::path::Path;
use std::{fmt, fmt::Display};
use rayon::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FTypeError {
    #[error("No version information was found in the file")]
    NoVersion,
    #[error("Version information appears to be invalid")]
    InvalidVersion,
}

#[derive(Error, Debug)]
pub enum FtvFileError {
    #[error("There was an error while trying to access the file")]
    IoError(#[from] std::io::Error),

    #[error("The file does not appear to be a valid FactoryTalk View ME File: {0:?}")]
    FileTypeError(#[from] FTypeError),
}

/// Holds the version number of the file.
#[derive(Debug, Default, Eq, PartialEq, Clone)]
pub struct FileVersion {
    /// Major Revision Number
    major_rev: u8,
    /// Minor Revision Number
    minor_rev: u8,
}

impl Display for FileVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major_rev, self.minor_rev)
    }
}

impl FileVersion {
    pub fn is_old(&self) -> bool {
        self.major_rev < 5
    }

    pub fn is_restorable(&self) -> bool {
        self.major_rev >= 4
    }
}



// https://rust-lang.github.io/rust-clippy/master/index.html#missing_errors_doc
/// Returns a `FactoryTalk` View File Version for the file passed into it.
///
/// # Arguments
///
/// * `filename` - A path to the file to be checked
///
/// # Examples
///
/// ```
/// use ab_versions::get_version;
/// let file_version = get_version(&path_to_file).unwrap();
/// ```
///
/// # Errors
///
/// Will return `Err`  if there is an error trying to access the file,
// or the file is invalid.
pub fn get_version<P: AsRef<Path>>(filename: &P) -> Result<FileVersion, FtvFileError> {
    let mut file = cfb::open(filename)?;

    let version_data = {
        let mut stream =
            file.open_stream("/VERSION_INFORMATION")
                .map_err(|err| -> FtvFileError {
                    match err.kind() {
                        std::io::ErrorKind::NotFound => FTypeError::NoVersion.into(),
                        _ => err.into(),
                    }
                })?;
        let mut buffer = Vec::new();
        stream.read_to_end(&mut buffer)?;
        buffer
    };

    if version_data.len() == 3 {
        Ok(FileVersion {
            major_rev: version_data[1],
            minor_rev: version_data[2],
        })
    } else {
        Err(FTypeError::InvalidVersion.into())
    }
}

// https://rust-lang.github.io/rust-clippy/master/index.html#missing_errors_doc
/// Returns a `FactoryTalk` View File Version for the file passed into it.
///
/// # Arguments
///
/// * `files` - A slice of paths to the files to be checked
///
/// # Examples
///
/// ```
/// use ab_versions::get_versions;
/// let file_version = get_versions(&paths_to_files).unwrap();
/// ```
///
/// # Errors
///
/// Will return `Err`  if there is an error trying to access the file,
// or the file is invalid.
//this parrallel version is mainly for the python bindings to take advantage
//or the parrallelization
pub fn get_versions<P>(files: &[P]) -> Vec<Result<FileVersion, FtvFileError>>
    where P: AsRef<Path> + Sync
{
    files.as_parallel_slice().par_iter().map(|file| -> Result<FileVersion, FtvFileError> {
        get_version(file)
    }).collect()
}


// https://rust-lang.github.io/rust-clippy/master/index.html#missing_errors_doc
/// Returns true/false if a `FactoryTalk` View file (APA or MER) is protected.
/// Protected could mean password protected, or locked and set to "Never restore" for an MER
///
/// # Arguments
///
/// * `path` - A path to the file to be checked
///
/// # Examples
///
/// ```
/// use ab_versions::is_protected;
/// let protected = is_protected(&path_to_file).unwrap();
/// ```
///
/// # Errors
///
/// Will return `Err`  if there is an error trying to access the file,
// or the file is invalid.
pub fn is_protected<P: AsRef<Path>>(path: &P) -> Result<bool, FtvFileError> {
    let mut file = cfb::open(path)?;

    let mut prot_stream = file.open_stream("/FILE_PROTECTION")?;

    // I'm not quite sure exactly what the contents of the file is if it is unprotected
    // So far has always been 7 bytes, and the the second byte has always been a 3,
    // and the rest have been 0. If it's password protected it's always been greater than
    // 7 bytes. I assume it's some hashed form of the password.
    // The exception here is if when an MER is exported with the "Never Convert" option selected
    // the bytes pattern seems to always be: [00, 03, 00, 01, 00, 00, 00], pretty similar to the
    // unlocked bytes, but with the 4th byte set to 1.

    Ok(if prot_stream.len() == 7 {
        let mut buf: Vec<u8> = Vec::with_capacity(7);
        prot_stream.read_to_end(&mut buf)?;
        buf == [0x00, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00]
    } else {
        prot_stream.len() > 7
    })
}

// https://rust-lang.github.io/rust-clippy/master/index.html#missing_errors_doc
/// Returns true/false if a `FactoryTalk` View file (APA or MER) is protected.
/// Protected could mean password protected, or locked and set to "Never restore" for an MER
///
/// # Arguments
///
/// * `files` - A slice of paths to the files to be checked
///
/// # Examples
///
/// ```
/// use ab_versions::are_protected;
/// let protected = are_protected(&paths_to_files).unwrap();
/// ```
///
/// # Errors
///
/// Will return `Err`  if there is an error trying to access the file,
// or the file is invalid.
//this parrallel version is mainly for the python bindings to take advantage
//or the parrallelization
pub fn are_protected<P>(files: &[P]) -> Vec<Result<bool, FtvFileError>>
    where P: AsRef<Path> + Sync
{
    files.as_parallel_slice().par_iter().map(|file| -> Result<bool, FtvFileError> {
        is_protected(file)
    }).collect()
}

// https://rust-lang.github.io/rust-clippy/master/index.html#missing_errors_doc
/// Strips the password protection or "Never Convert" setting of an `FactoryTalk` View
/// MER or APA file
///
/// # Arguments
///
/// * `path` - A path to the file to be checked
///
/// # Examples
///
/// ```
/// use ab_versions::strip_protection;
/// strip_protection(&path_to_file).unwrap();
/// ```
///
/// # Errors
///
/// Will return `Err`  if there is an error trying to access the file,
// or the file is invalid.
pub fn strip_protection<P: AsRef<Path>>(path: P) -> Result<(), FtvFileError> {
    // Note from what I recall of my earlier testing, removing the stream "/FILE_PROTECTION"
    // or setting it to a single byte of 0, or 7 bytes of 0, also removed the protection and
    // caused no problems that I could tell. To err on the side of caution, it seemed safter
    //to leave the "/FILE_PROTECTION" stream in place and set it's byte to the 7 byte pattern
    //that seems to be used for all unlocked files.

    // Also of note if an MER is set to "Never Convert" stripping the protection this way will work
    // and allows the MER to be restored. I've tested this on MERs from version 12 all the way down to
    // MER Version 5 which doesn't give you a choice and is always "Never Convert", and it always seems
    // to work

    //Ok so I never had access to a file with version lower than 5, but my research told me that they didn't contain
    //the info to be restored to a project file and even this method of stripping the "File Protection"
    //wouldn't work. I finally got some v4 files (although I can't include them in the test suite)
    //and they actually do seem to have the information to convert them, but the process is slightly different
    //First I need to CREATE the FILE_PROTECTION stream, and set it to an unlocked value
    //then I need to modify the VERSION_INFORMATION to look like a newer version (I'll use v5.10)
    //no clue if this actually works on anything < v4 since I have nothing to test against


    let mut file = cfb::open_rw(&path)?;

    let version = get_version(&path)?;

    if version.major_rev < 5 {
        //if version < 5 use other method, to create FILE_PROTECTION
        //and set the file_version to 5.10
        //wonder if I should be checking for a .med stream to verify it's an MER here?
        let mut fp_stream = file.create_new_stream("/FILE_PROTECTION")?;
        fp_stream.set_len(7)?;
        fp_stream.write_all(&[0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00])?;

        let mut ver_stream = file.open_stream("/VERSION_INFORMATION")?;
        ver_stream.write_all(&[0x03, 0x05, 0x0A])?;

    } else {
        //if version >= 5 use normal method just stip protection
        let mut stream = file.open_stream("/FILE_PROTECTION")?;
        stream.set_len(7)?;
        stream.write_all(&[0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00])?;
    }

    Ok(())
}

// https://rust-lang.github.io/rust-clippy/master/index.html#missing_errors_doc
/// Strips the password protection or "Never Convert" setting of an `FactoryTalk` View
/// MER or APA file
///
/// # Arguments
///
/// * `files` - A slice of paths to the files to be checked
///
/// # Examples
///
/// ```
/// use ab_versions::strip_protections;
/// strip_protection(&paths_to_files).unwrap();
/// ```
///
/// # Errors
///
/// Will return `Err`  if there is an error trying to access the file,
// or the file is invalid.
//this parrallel version is mainly for the python bindings to take advantage
//or the parrallelization
pub fn strip_protections<P>(files: &[P]) -> Result<(), FtvFileError>
    where P: AsRef<Path> + Sync
{
    // Note from what I recall of my earlier testing, removing the stream "/FILE_PROTECTION"
    // or setting it to a single byte of 0, or 7 bytes of 0, also removed the protection and
    // caused no problems that I could tell. To err on the side of caution, it seemed safter
    //to leave the "/FILE_PROTECTION" stream in place and set it's byte to the 7 byte pattern
    //that seems to be used for all unlocked files.

    // Also of note if an MER is set to "Never Convert" stripping the protection this way will work
    // and allows the MER to be restored. I've tested this on MERs from version 12 all the way down to
    // MER Version 5 which doesn't give you a choice and is always "Never Convert", and it always seems
    // to work

    files.as_parallel_slice().par_iter().map(|file| -> Result<(), FtvFileError> {
        strip_protection(file)
    }).collect()
}

#[cfg(test)]
mod tests {
    use nom::{
        bytes::complete::{tag, take_until},
        character::complete::digit1,
        combinator::map_res,
        IResult,
    };

    use crate::strip_protection;
    enum FileState {
        Unlocked,
        Locked,
        Never,
    }

    fn string_u8(input: &str) -> IResult<&str, u8> {
        map_res(digit1, str::parse)(input)
    }

    // Gets the version number the test should return if successfully stripped from the file name
    fn file_name_version(input: &str) -> IResult<&str, super::FileVersion> {
        let (input, _) = tag("_V")(input)?;
        let (input, major_rev) = string_u8(input)?;
        let (input, _) = tag("-")(input)?;
        let (input, minor_rev) = string_u8(input)?;

        Ok((
            input,
            super::FileVersion {
                major_rev,
                minor_rev,
            },
        ))
    }

    fn strip_version(input: &str) -> IResult<&str, super::FileVersion> {
        let (input, _) = take_until("_V")(input)?;
        file_name_version(input)
    }

    fn process_archive<P: AsRef<std::path::Path>>(archive_path: P, state: &FileState) {
        use super::*;
        use tempfile::tempdir;
        use walkdir::WalkDir;
        use sevenz_rust::decompress_file;

        let extract_dir = tempdir().expect(
            "Test failed due to inability to create a temporary directory to uncompress the files",
        );

        decompress_file(&archive_path, extract_dir.path())
            .expect("Test failed due to inability to extract the test files from the archive");

        let walker = WalkDir::new(extract_dir.path()).into_iter();
        walker
            .filter_map(std::result::Result::ok)
            .filter(|en| {
                en.file_name().to_str().map_or(false, |s| {
                    s.rsplit('.')
                        .next()
                        .map(|ext| ext.eq_ignore_ascii_case("mer"))
                        == Some(true)
                        || s.rsplit('.')
                            .next()
                            .map(|ext| ext.eq_ignore_ascii_case("apa"))
                            == Some(true)
                })
            })
            .for_each(|entry| {
                //TODO: Need to make sure I get some APAs in the archive test files
                //TODO: Need to finish creating the remaining MERs and add them to the archive test Files
                let strip_fail = format!(
                    "Unable to determine file version from file name to use in test: {:?}",
                    &entry.path()
                );
                let (_, file_name_version) =
                    strip_version(entry.file_name().to_str().unwrap()).expect(&strip_fail);

                assert_eq!(file_name_version, get_version(&entry.path()).unwrap());

                match state {
                    FileState::Unlocked => {
                        assert!(!is_protected(&entry.path()).unwrap());
                    }
                    FileState::Locked | FileState::Never => {
                        assert!(is_protected(&entry.path()).unwrap()); // File should return locked

                        strip_protection(entry.path()).unwrap();

                        assert!(!is_protected(&entry.path()).unwrap()); // File should return unlocked after we try to unlock it
                    }
                }
            });
    }

    //TODO: Need to fix the V11 mer that's labeled as V12. Looks like I misexported it. Will need to generate a new one
    #[test]
    fn unlocked_file() {
        process_archive("./test_files/Unlocked.7z", &FileState::Unlocked);
    }

    #[test]
    fn locked_file() {
        process_archive("./test_files/Locked.7z", &FileState::Locked);
    }

    #[test]
    fn never_file() {
        process_archive("./test_files/Never.7z", &FileState::Never);
    }
}