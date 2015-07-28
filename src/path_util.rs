use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::path::Component::RootDir;

trait ToRelative {
    fn rel<'x>(&'x self) -> &'x Path;
    fn rel_to<'x>(&'x self, &Path) -> Option<&'x Path>;
}

impl ToRelative for Path {
    fn rel<'x>(&'x self) -> &'x Path {
        let mut iter = self.components();
        assert!(iter.next() == Some(RootDir));
        iter.as_path()
    }
    fn rel_to<'x>(&'x self, other: &Path) -> Option<&'x Path> {
        let mut iter = self.components();
        for (my, their) in iter.as_ref() {
            if my != their {
                return None;
            }
        }
        Some(iter.as_path())
    }
}

impl ToRelative for PathBuf {
    fn rel<'x>(&'x self) -> &'x Path {
        self.as_path().rel()
    }
    fn rel_to<'x>(&'x self, other: &Path) -> Option<&'x Path> {
        self.as_path().rel_to(other)
    }
}

trait ToCString {
    fn to_cstring(&self) -> CString;
}

impl ToCString for Path {
    fn to_cstring(&self) -> CString {
        CString::new(self.as_os_str().as_bytes()).unwrap()
    }
}

impl<'a> ToCString for &'a str {
    fn to_cstring(&self) -> CString {
        CString::new(self.as_bytes()).unwrap()
    }
}
