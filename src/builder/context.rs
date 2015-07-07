use std::fs::{create_dir_all, create_dir, copy, set_permissions};
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::default::Default;
use std::collections::{BTreeMap, BTreeSet};

use container::mount::{bind_mount, unmount, mount_system_dirs};
use container::util::clean_dir;
use config::Config;
use config::Container;
use config::Settings;
use config::builders::PipSettings;
use super::commands::debian::UbuntuInfo;
use super::commands::alpine::AlpineInfo;
use super::commands::debian;
use super::commands::alpine;
use super::commands::pip;
use super::commands::npm;
use super::capsule;
use super::packages;
use super::timer;

#[derive(Debug)]
pub enum Distribution {
    Unknown,
    Ubuntu(UbuntuInfo),
    Alpine(AlpineInfo),
}

pub struct BuildContext<'a> {
    pub config: &'a Config,
    pub container_name: String,
    pub container_config: &'a Container,
    ensure_dirs: BTreeSet<PathBuf>,
    empty_dirs: BTreeSet<PathBuf>,
    remove_dirs: BTreeSet<PathBuf>,
    cache_dirs: BTreeMap<PathBuf, String>,
    pub environ: BTreeMap<String, String>,

    pub settings: Settings,
    pub distribution: Distribution,
    pub pip_settings: PipSettings,
    pub capsule: capsule::State,
    pub packages: BTreeSet<String>,
    pub build_deps: BTreeSet<String>,
    pub featured_packages: BTreeSet<packages::Package>,
    pub timelog: timer::TimeLog,
}

impl<'a> BuildContext<'a> {
    pub fn new<'x>(cfg: &'x Config, name: String,
        container: &'x Container, settings: Settings)
        -> BuildContext<'x>
    {
        return BuildContext {
            config: cfg,
            container_name: name,
            container_config: container,
            ensure_dirs: vec!(
                PathBuf::from("proc"),
                PathBuf::from("sys"),
                PathBuf::from("dev"),
                PathBuf::from("work"),
                PathBuf::from("tmp"),
                ).into_iter().collect(),
            empty_dirs: vec!(
                PathBuf::from("tmp"),
                PathBuf::from("var/tmp"),
                ).into_iter().collect(),
            remove_dirs: vec!(
                ).into_iter().collect(),
            cache_dirs: vec!(
                ).into_iter().collect(),
            environ: vec!(
                ("TERM".to_string(), "dumb".to_string()),
                ("HOME".to_string(), "/tmp".to_string()),
                ("PATH".to_string(),
                 "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
                 .to_string()),
                ).into_iter().collect(),

            settings: settings,
            distribution: Distribution::Unknown,
            pip_settings: Default::default(),
            capsule: Default::default(),
            packages: BTreeSet::new(),
            build_deps: BTreeSet::new(),
            featured_packages: BTreeSet::new(),
            timelog: timer::TimeLog::start("/vagga/container/timings.log")
                .map_err(|e| format!("Can't write timelog: {}", e))
                .unwrap(),
        };
    }

    pub fn add_cache_dir(&mut self, path: &Path, name: String)
        -> Result<(), String>
    {
        assert!(path.is_absolute());
        let path = path.path_relative_from(&Path::new("/")).unwrap();
        if self.cache_dirs.insert(path.clone(), name.clone()).is_none() {
            let cache_dir = Path::new("/vagga/cache").join(name.as_slice());
            if !cache_dir.exists() {
                try!(create_dir(&cache_dir)
                     .map_err(|e| format!("Error creating cache dir: {}", e)));
                try!(set_permissions(&cache_dir,
                    Permissions::from_mode(0o755)));
            }
            let path = Path::new("/vagga/root").join(path);
            try!(create_dir_all(&path)
                 .map_err(|e| format!("Error creating cache dir: {}", e)));
            try!(set_permissions(&path,
                Permissions::from_mode(0o755)));
            try!(clean_dir(&path, false));
            try!(bind_mount(&cache_dir, &path));
        }
        return Ok(());
    }

    pub fn add_remove_dir(&mut self, path: Path) {
        assert!(path.is_absolute());
        let path = path.path_relative_from(&Path::new("/")).unwrap();
        self.remove_dirs.insert(path);
    }

    pub fn add_empty_dir(&mut self, path: Path) {
        assert!(path.is_absolute());
        let path = path.path_relative_from(&Path::new("/")).unwrap();
        self.empty_dirs.insert(path);
    }

    pub fn add_ensure_dir(&mut self, path: Path) {
        assert!(path.is_absolute());
        let path = path.path_relative_from(&Path::new("/")).unwrap();
        self.ensure_dirs.insert(path);
    }
    pub fn start(&mut self) -> Result<(), String> {
        try!(mount_system_dirs());
        try!(create_dir("/vagga/root/etc")
             .map_err(|e| format!("Error creating /etc dir: {}", e)));
        try!(set_permissions("/vagga/root/etc", Permissions::from_mode(0o755))
             .map_err(|e| format!("Error chmod /etc dir: {}", e)));
        try!(copy("/etc/resolv.conf", "/vagga/root/etc/resolv.conf")
            .map_err(|e| format!("Error copying /etc/resolv.conf: {}", e)));
        try!(self.timelog.mark(format_args!("Prepare"))
            .map_err(|e| format!("Can't write timelog: {}", e)));
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), String> {
        if self.featured_packages.contains(&packages::PipPy2) ||
           self.featured_packages.contains(&packages::PipPy3)
        {
            try!(pip::freeze(self));
        }
        if self.featured_packages.contains(&packages::Npm) {
            try!(npm::list(self));
        }

        match self.distribution {
            Distribution::Unknown => {}
            Distribution::Ubuntu(_) => {
                try!(debian::finish(self));
            }
            Distribution::Alpine(_) => {
                try!(alpine::finish(self));
            }
        }

        let base = Path::new("/vagga/root");

        for (dir, _) in self.cache_dirs.iter().rev() {
            try!(unmount(&base.join(dir)));
        }

        for dir in self.remove_dirs.iter() {
            try!(clean_dir(&base.join(dir), false)
                .map_err(|e| format!("Error removing dir: {}", e)));
        }

        for dir in self.empty_dirs.iter() {
            try!(clean_dir(&base.join(dir), false));
        }

        for dir in self.ensure_dirs.iter() {
            let fulldir = base.join(dir);
            try!(create_dir_all(&fulldir)
                .map_err(|e| format!("Error creating dir: {}", e)));
            try!(set_permissions(&fulldir, Permissions::from_mode(0o755))
                .map_err(|e| format!("Error chmod dir: {}", e)));
        }

        try!(self.timelog.mark(format_args!("Finish"))
            .map_err(|e| format!("Can't write timelog: {}", e)));

        return Ok(());
    }
}
