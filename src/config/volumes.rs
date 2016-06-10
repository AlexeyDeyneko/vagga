use std::path::PathBuf;
use std::collections::BTreeMap;

use quire::validate as V;
use libc::{uid_t, gid_t};


#[derive(RustcDecodable, Clone, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub size: usize,
    pub owner_uid: Option<uid_t>,
    pub owner_gid: Option<gid_t>,
}

#[derive(RustcDecodable, Clone, PartialEq, Eq)]
pub enum Volume {
    Tmpfs(TmpfsInfo),
    BindRW(PathBuf),
    BindRO(PathBuf),
    Empty,
    VaggaBin,
    Snapshot(SnapshotInfo),
    Container(String),
}

#[derive(RustcDecodable, Clone, PartialEq, Eq)]
pub struct Dir {
    pub mode: u32,
}

#[derive(RustcDecodable, Clone, PartialEq, Eq)]
pub struct TmpfsInfo {
    pub size: usize,
    pub mode: u32,
    pub subdirs: BTreeMap<PathBuf, Dir>,
}

pub fn volume_validator<'x>() -> V::Enum<'x> {
    V::Enum::new()
    .option("Tmpfs",  V::Structure::new()
        .member("size",  V::Numeric::new()
            .min(0).default(100*1024*1024))
        .member("mode",  V::Numeric::new()
            .min(0).max(0o1777).default(0o1777))
        .member("subdirs",
            V::Mapping::new(
                V::Directory::new().is_absolute(false),
                V::Structure::new()
                    .member("mode", V::Numeric::new()
                        .min(0).max(0o1777).default(0o755))
            )))
    .option("VaggaBin",  V::Nothing)
    .option("BindRW",  V::Scalar::new())
    .option("BindRO",  V::Scalar::new())
    .option("Empty",  V::Nothing)
    .option("Snapshot",  V::Structure::new()
        .member("size",  V::Numeric::new().min(0).default(100*1024*1024))
        .member("owner_uid", V::Numeric::new().min(0).optional())
        .member("owner_gid", V::Numeric::new().min(0).optional())
        )
    .option("Container",  V::Scalar::new())
}
