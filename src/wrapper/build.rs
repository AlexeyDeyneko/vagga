use std::rc::Rc;
use std::os::{Pipe, pipe};
use std::io::PipeStream;
use std::io::ALL_PERMISSIONS;
use std::os::{getenv};
use std::cell::RefCell;
use std::io::fs::{rmdir_recursive, mkdir_recursive, mkdir, rename};
use std::io::fs::PathExtensions;
use std::io::stdio::{stdout, stderr};
use libc::funcs::posix88::unistd::close;
use serialize::json;

use argparse::{ArgumentParser, Store, StoreTrue};

use container::mount::{bind_mount};
use container::monitor::{Monitor, Executor, MonitorStatus, Shutdown};
use container::monitor::{Killed, Exit};
use container::container::{Command};
use container::uidmap::{map_users, Ranges, Singleton};
use config::{Settings, Config};


struct RunBuilder<'a> {
    container: String,
    settings: &'a Settings,
}

struct RunVersion<'a> {
    container: String,
    pipe: Pipe,
    result: Rc<RefCell<String>>,
    settings: &'a Settings,
}

impl<'a> Executor for RunVersion<'a> {
    fn command(&mut self) -> Command {
        let mut cmd = Command::new("vagga_version".to_string(),
            Path::new("/vagga/bin/vagga_version"));
        cmd.keep_sigmask();
        cmd.set_uidmap(self.settings.uid_map.as_ref()
            .map(|&(ref x, ref y)| Ranges(x.clone(), y.clone()))
            .unwrap_or(Singleton(0, 0)));
        cmd.arg(self.container.as_slice());
        cmd.arg("--settings");
        cmd.arg(json::encode(self.settings));
        cmd.set_env("TERM".to_string(), "dumb".to_string());
        cmd.set_stdout_fd(self.pipe.writer);
        if let Some(x) = getenv("RUST_LOG") {
            cmd.set_env("RUST_LOG".to_string(), x);
        }
        if let Some(x) = getenv("RUST_BACKTRACE") {
            cmd.set_env("RUST_BACKTRACE".to_string(), x);
        }
        return cmd;
    }
    fn finish(&mut self, status: int) -> MonitorStatus {
        unsafe { close(self.pipe.writer) };
        if status == 0 {
            let mut rd = PipeStream::open(self.pipe.reader);
            *self.result.borrow_mut() = rd.read_to_string()
                                          .unwrap_or("".to_string());
        } else {
            unsafe { close(self.pipe.reader) };
        }
        return Shutdown(status)
    }
}

impl<'a> Executor for RunBuilder<'a> {
    fn command(&mut self) -> Command {
        let mut cmd = Command::new("vagga_build".to_string(),
            Path::new("/vagga/bin/vagga_build"));
        cmd.keep_sigmask();
        cmd.set_uidmap(self.settings.uid_map.as_ref()
            .map(|&(ref x, ref y)| Ranges(x.clone(), y.clone()))
            .unwrap_or(Singleton(0, 0)));
        cmd.container();
        cmd.arg(self.container.as_slice());
        cmd.arg("--settings");
        cmd.arg(json::encode(self.settings));
        cmd.set_env("TERM".to_string(),
                    getenv("TERM").unwrap_or("dumb".to_string()));
        if let Some(x) = getenv("RUST_LOG") {
            cmd.set_env("RUST_LOG".to_string(), x);
        }
        if let Some(x) = getenv("RUST_BACKTRACE") {
            cmd.set_env("RUST_BACKTRACE".to_string(), x);
        }
        return cmd;
    }
}

pub fn prepare_tmp_root_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        try!(rmdir_recursive(path)
             .map_err(|x| format!("Error creating directory: {}", x)));
    }
    try!(mkdir_recursive(path, ALL_PERMISSIONS)
         .map_err(|x| format!("Error creating directory: {}", x)));
    let rootdir = path.join("root");
    try!(mkdir(&rootdir, ALL_PERMISSIONS)
         .map_err(|x| format!("Error creating directory: {}", x)));
    let tgtroot = Path::new("/vagga/root");
    try!(mkdir(&tgtroot, ALL_PERMISSIONS)
         .map_err(|x| format!("Error creating directory: {}", x)));
    try!(bind_mount(&rootdir, &tgtroot));
    try!(mkdir(&tgtroot.join("dev"), ALL_PERMISSIONS)
         .map_err(|x| format!("Error creating directory: {}", x)));
    try!(mkdir(&tgtroot.join("sys"), ALL_PERMISSIONS)
         .map_err(|x| format!("Error creating directory: {}", x)));
    try!(mkdir(&tgtroot.join("proc"), ALL_PERMISSIONS)
         .map_err(|x| format!("Error creating directory: {}", x)));
    try!(mkdir(&tgtroot.join("work"), ALL_PERMISSIONS)
         .map_err(|x| format!("Error creating directory: {}", x)));
    return Ok(());
}

pub fn commit_root(tmp_path: &Path, final_path: &Path) -> Result<(), String> {
    let mut path_to_remove = None;
    if final_path.exists() {
        let rempath = tmp_path.with_filename(
            tmp_path.filename_str().unwrap().to_string() + ".old");
        try!(rename(final_path, &rempath)
             .map_err(|x| format!("Error renaming old dir: {}", x)));
        path_to_remove = Some(rempath);
    }
    try!(rename(tmp_path, final_path)
         .map_err(|x| format!("Error renaming dir: {}", x)));
    if let Some(ref path_to_remove) = path_to_remove {
        try!(rmdir_recursive(path_to_remove)
             .map_err(|x| format!("Error removing old dir: {}", x)));
    }
    return Ok(());
}

pub fn get_version_hash(container: String, settings: &Settings)
    -> Result<Option<String>, String>
{
    let mut mon = Monitor::new();
    let ver = Rc::new(RefCell::new("".to_string()));
    mon.add(Rc::new("version".to_string()), box RunVersion {
        container: container,
        pipe: unsafe { pipe() }.ok().expect("Can't create pipe"),
        result: ver.clone(),
        settings: settings,
    });
    match mon.run() {
        Killed => return Err(format!("Versioner has died")),
        Exit(0) => {},
        Exit(29) => return Ok(None),
        Exit(val) => return Err(format!("Versioner exited with code {}", val)),
    };
    return Ok(Some(ver.borrow().to_string()));
}

pub fn build_container(container: &String, force: bool, settings: &Settings)
    -> Result<String, String>
{
    let mut mon = Monitor::new();
    let ver = Rc::new(RefCell::new("".to_string()));
    mon.add(Rc::new("version".to_string()), box RunVersion {
        container: container.clone(),
        pipe: unsafe { pipe() }.ok().expect("Can't create pipe"),
        result: ver.clone(),
        settings: settings,
    });
    match mon.run() {
        Killed => return Err(format!("Builder has died")),
        Exit(0) if force => {}
        Exit(0) => {
            debug!("Container version: {}", ver.borrow());
            let name = format!("{}.{}", container,
                ver.borrow().as_slice().slice_to(8));
            let finalpath = Path::new("/vagga/roots")
                .join(name.as_slice());
            if finalpath.exists() {
                debug!("Path {} is already built",
                       finalpath.display());
                return Ok(name);
            }
        },
        Exit(29) => {},
        Exit(val) => return Err(format!("Builder exited with code {}", val)),
    };
    debug!("Container version: {}", ver.borrow());
    let tmppath = Path::new(format!("/vagga/roots/.tmp.{}", container));
    match prepare_tmp_root_dir(&tmppath) {
        Ok(()) => {}
        Err(x) => {
            return Err(format!("Error preparing root dir: {}", x));
        }
    }
    mon.add(Rc::new("build".to_string()), box RunBuilder {
        container: container.to_string(),
        settings: settings,
    });
    match mon.run() {
        Killed => return Err(format!("Builder has died")),
        Exit(0) => {},
        Exit(val) => return Err(format!("Builder exited with code {}", val)),
    };
    if ver.borrow().len() != 64 {
        mon.add(Rc::new("version".to_string()), box RunVersion {
            container: container.to_string(),
            pipe: unsafe { pipe() }.ok().expect("Can't create pipe"),
            result: ver.clone(),
            settings: settings,
        });
        match mon.run() {
            Killed => return Err(format!("Builder has died")),
            Exit(0) => {},
            Exit(29) => {
                return Err(format!("Internal Error: \
                        Can't version even after build"));
            },
            Exit(val) => return Err(format!("Builder exited with code {}",
                                    val)),
        };
    }
    let name = format!("{}.{}", container,
        ver.borrow().as_slice().slice_to(8));
    let finalpath = Path::new("/vagga/roots").join(name.as_slice());
    debug!("Committing {} -> {}", tmppath.display(),
                                  finalpath.display());
    match commit_root(&tmppath, &finalpath) {
        Ok(()) => {}
        Err(x) => {
            return Err(format!("Error committing root dir: {}", x));
        }
    }
    return Ok(name);
}

pub fn build_container_cmd(config: &Config, settings: &Settings,
    cmdline: Vec<String>)
    -> Result<int, String>
{
    let mut name: String = "".to_string();
    let mut force: bool = false;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("
            Internal vagga tool to setup basic system sandbox
            ");
        ap.refer(&mut name)
            .add_argument("container_name", box Store::<String>,
                "Container name to build");
        ap.refer(&mut force)
            .add_option(&["--force"], box StoreTrue,
                "Force build even if container is considered up to date");
        match ap.parse(cmdline, &mut stdout(), &mut stderr()) {
            Ok(()) => {}
            Err(0) => return Ok(0),
            Err(_) => {
                return Ok(122);
            }
        }
    }
    let container = try!(config.containers.find(&name)
        .ok_or(format!("Container {} not found", name)));
    let settings = try!(map_users(settings, &container.uids, &container.gids));
    return build_container(&name, force, &settings)
        .map(|x| debug!("Built container with name {}", x))
        .map(|()| 0);
}

pub fn print_version_hash_cmd(config: &Config, settings: &Settings,
    cmdline: Vec<String>)
    -> Result<int, String>
{
    let mut name: String = "".to_string();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("
            Prints version hash of the container without building it. If
            this command exits with code 29, then container can't be versioned
            before the build.
            ");
        ap.refer(&mut name)
            .add_argument("container_name", box Store::<String>,
                "Container name to build");
        match ap.parse(cmdline, &mut stdout(), &mut stderr()) {
            Ok(()) => {}
            Err(0) => return Ok(0),
            Err(_) => {
                return Ok(122);
            }
        }
    }
    let container = try!(config.containers.find(&name)
        .ok_or(format!("Container {} not found", name)));
    let settings = try!(map_users(settings, &container.uids, &container.gids));
    return get_version_hash(name, &settings)
        .map(|ver| ver
            .map(|x| println!("{}", x)).map(|()| 0)
            .unwrap_or(29));
}

