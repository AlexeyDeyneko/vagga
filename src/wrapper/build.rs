use std::env;
use std::fs::{File, read_link};
use std::ffi::OsString;
use std::io::ErrorKind::NotFound;
use std::ascii::AsciiExt;
use std::fs::{rename};
use std::fs::{remove_file, remove_dir};
use std::io::{self, Read, Write};
use std::io::{stdout, stderr};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::time::{Instant};
use std::os::unix::io::FromRawFd;

use argparse::{ArgumentParser, Store, StoreTrue};
use rustc_serialize::json;
use unshare::{Command, Namespace, ExitStatus};
use libmount::BindMount;

use container::util::clean_dir;
use container::mount::{unmount};
use file_util::{Dir, Lock};
use process_util::{capture_fd3_status, copy_env_vars};
use super::Wrapper;
use super::setup;
use build_step::Step;
use options::version_hash::Options;


pub fn prepare_tmp_root_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        clean_dir(path, true)
             .map_err(|x| format!("Error removing directory: {}", x))?;
    }
    try_msg!(Dir::new(path).recursive(true).create(),
         "Error creating directory: {err}");
    let rootdir = path.join("root");
    try_msg!(Dir::new(&rootdir).create(),
         "Error creating directory: {err}");

    let tgtbase = Path::new("/vagga/container");
    try_msg!(Dir::new(&tgtbase).create(),
         "Error creating directory: {err}");
    try_msg!(BindMount::new(path, &tgtbase).mount(),
        "mount container: {err}");

    let tgtroot = Path::new("/vagga/root");
    try_msg!(Dir::new(&tgtroot).create(),
         "Error creating directory: {err}");
    try_msg!(BindMount::new(&rootdir, &tgtroot).mount(),
        "mount container root: {err}");

    try_msg!(Dir::new(&tgtroot.join("dev")).create(),
         "Error creating directory: {err}");
    try_msg!(Dir::new(&tgtroot.join("sys")).create(),
         "Error creating directory: {err}");
    try_msg!(Dir::new(&tgtroot.join("proc")).create(),
         "Error creating directory: {err}");
    try_msg!(Dir::new(&tgtroot.join("run")).create(),
         "Error creating directory: {err}");
    try_msg!(Dir::new(&tgtroot.join("tmp")).mode(0o1777).create(),
         "Error creating directory: {err}");
    try_msg!(Dir::new(&tgtroot.join("work")).create(),
         "Error creating directory: {err}");
    return Ok(());
}

pub fn commit_root(tmp_path: &Path, final_path: &Path) -> Result<(), String> {
    let mut path_to_remove = None;
    if final_path.exists() {
        let rempath = tmp_path.with_file_name(
            // TODO(tailhook) consider these unwraps
            tmp_path.file_name().unwrap().to_str()
            .unwrap().to_string() + ".old");
        if rempath.is_dir() {
            clean_dir(&rempath, true)
                 .map_err(|x| format!("Error removing old dir: {}", x))?;
        }
        rename(final_path, &rempath)
             .map_err(|x| format!("Error renaming old dir: {}", x))?;
        path_to_remove = Some(rempath);
    }
    rename(tmp_path, final_path)
         .map_err(|x| format!("Error renaming dir: {}", x))?;
    if let Some(ref path_to_remove) = path_to_remove {
        clean_dir(path_to_remove, true)
             .map_err(|x| format!("Error removing old dir: {}", x))?;
    }
    return Ok(());
}

pub fn get_version_hash(container: &str, wrapper: &Wrapper)
    -> Result<Option<String>, String>
{
    _get_version_hash(&Options::for_container(container), wrapper)
}

fn _get_version_hash(options: &Options, wrapper: &Wrapper)
    -> Result<Option<String>, String>
{
    let mut cmd = Command::new("/vagga/bin/vagga");
    cmd.arg("__version__");
    cmd.gid(0);
    cmd.groups(Vec::new());
    cmd.arg(&options.container);
    cmd.arg("--settings");
    cmd.arg(json::encode(wrapper.settings).unwrap());
    if options.debug_versioning {
        cmd.arg("--debug-versioning");
    }
    if options.dump_version_data {
        cmd.arg("--dump-version-data");
    }
    cmd.env_clear();
    copy_env_vars(&mut cmd, &wrapper.settings);
    if let Ok(x) = env::var("RUST_LOG") {
        cmd.env("RUST_LOG", x);
    }
    if let Ok(x) = env::var("RUST_BACKTRACE") {
        cmd.env("RUST_BACKTRACE", x);
    }
    if let Ok(x) = env::var("VAGGA_DEBUG_CMDENV") {
        cmd.env("VAGGA_DEBUG_CMDENV", x);
    }
    match capture_fd3_status(cmd) {
        Ok((ExitStatus::Exited(0), val)) => {
            String::from_utf8(val)
                .map_err(|e| format!("Can't decode version: {}", e))
                .map(Some)
        },
        Ok((ExitStatus::Exited(29), _)) => Ok(None),
        Ok((status, _)) => return Err(format!("Versioner exited {}", status)),
        Err(e) => return Err(format!("Could not run versioner: {}", e)),
    }
}

pub fn build_container(container: &str, force: bool, no_image: bool,
    wrapper: &Wrapper)
    -> Result<String, String>
{
    let cconfig = wrapper.config.containers.get(container)
        .ok_or(format!("Container {} not found", container))?;
    let name = _build_container(container, force, no_image, wrapper)?;
    let destlink = Path::new("/work/.vagga").join(&container);
    let tmplink = destlink.with_extension("tmp");
    if tmplink.exists() {
        remove_file(&tmplink)
            .map_err(|e| format!("Error removing temporary link: {}", e))?;
    }
    let roots = if wrapper.ext_settings.storage_dir.is_some() {
        Path::new(".lnk/.roots")
    } else {
        Path::new(".roots")
    };
    let linkval = roots.join(&name).join("root");
    if cconfig.auto_clean {
        match read_link(&destlink) {
            Ok(ref oldval) if oldval != &linkval => {
                let oldname = oldval.iter().rev().nth(1)
                    .ok_or(format!("Bad link {:?} -> {:?}",
                        destlink, oldval))?;
                let base = Path::new("/vagga/base/.roots");
                let dir = base.join(&oldname);
                let tmpdir = base.join({
                    let mut tmpname = OsString::from(".tmp");
                    tmpname.push(oldname);
                    tmpname
                });
                rename(&dir, &tmpdir)
                    .map_err(|e| error!("Error renaming old dir: {}", e)).ok();
                clean_dir(&tmpdir, true)
                    .map_err(|e| error!("Error removing old dir: {}", e)).ok();
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == NotFound => {}
            Err(e) => {
                return Err(format!("Error reading symlin {:?}: {}",
                    destlink, e));
            }
        };
    }
    symlink(&linkval, &tmplink)
         .map_err(|e| format!("Error symlinking container: {}", e))?;
    rename(&tmplink, &destlink)
         .map_err(|e| format!("Error renaming symlink: {}", e))?;
    return Ok(name);
}

fn compare_files<A: AsRef<Path>, B: AsRef<Path>>(a: A, b: B)
    -> io::Result<bool>
{
    let mut abuf = Vec::with_capacity(1024);
    let mut bbuf = Vec::with_capacity(1024);
    File::open(a.as_ref()).and_then(|mut f| f.read_to_end(&mut abuf))?;
    File::open(b.as_ref()).and_then(|mut f| f.read_to_end(&mut bbuf))?;
    Ok(abuf != bbuf)
}

fn uidmap_differs(container_path: &Path) -> bool {
    compare_files(
        "/proc/self/uid_map",
        container_path.join("uid_map")
    ).unwrap_or(true) ||
    compare_files(
        "/proc/self/uid_map",
        container_path.join("uid_map")
    ).unwrap_or(true)
}

pub fn _build_container(container: &str,
    force: bool, no_image: bool, wrapper: &Wrapper)
    -> Result<String, String>
{
    let tmppath = PathBuf::from(
        &format!("/vagga/base/.roots/.tmp.{}", container));

    let ver = match get_version_hash(container, wrapper) {
        Ok(Some(ver)) => {
            if ver.len() == 128 && ver[..].is_ascii() {
                let name = format!("{}.{}", container, &ver[..8]);
                let finalpath = Path::new("/vagga/base/.roots")
                    .join(&name);
                debug!("Container path: {:?} (force: {}) {}", finalpath, force,
                    finalpath.exists());
                if finalpath.exists() && !force {
                    if uidmap_differs(&finalpath) {
                        warn!("Current uidmap differs from uidmap of container \
                        when it was built.  This probably means that you \
                        either running vagga wrong user id or changed uid or \
                        subuids of your user since container was built. We'll \
                        rebuilt container to make sure it has proper \
                        permissions");
                    } else {
                        debug!("Path {:?} is already built", finalpath);
                        return Ok(name);
                    }
                }
                Some(ver)
            } else {
                error!("Wrong version hash: {:?}", ver);
                None
            }
        }
        Ok(None) => None,
        Err(e) => return Err(e),
    };
    debug!("Container version: {:?}", ver);

    let lock_name = tmppath.with_file_name(format!(".tmp.{}.lock", container));
    let mut _lock_guard = if wrapper.settings.build_lock_wait {
        Lock::exclusive_wait(&lock_name,
            "Other process is doing a build. Waiting...")
        .map_err(|e| format!("Can't lock container build ({}). \
                              Aborting...", e))?

    } else {
        Lock::exclusive(&lock_name)
        .map_err(|e| format!("Can't lock container build ({}). \
            Probably other process is doing build. Aborting...", e))?
    };

    match prepare_tmp_root_dir(&tmppath) {
        Ok(()) => {}
        Err(x) => {
            return Err(format!("Error preparing root dir: {}", x));
        }
    }

    let mut cmd = Command::new("/vagga/bin/vagga");
    cmd.arg("__builder__");
    cmd.gid(0);
    cmd.groups(Vec::new());
    cmd.unshare(
        [Namespace::Mount, Namespace::Ipc, Namespace::Pid].iter().cloned());
    cmd.arg(&container);
    if let Some(ref ver) = ver {
        cmd.arg("--container-version");
        cmd.arg(format!("{}.{}", container, &ver[..8]));
    }
    if force || no_image {
        cmd.arg("--no-image-download");
    }
    cmd.arg("--settings");
    cmd.arg(json::encode(wrapper.settings).unwrap());
    cmd.env_clear();
    copy_env_vars(&mut cmd, &wrapper.settings);
    if let Ok(x) = env::var("RUST_LOG") {
        cmd.env("RUST_LOG", x);
    }
    if let Ok(x) = env::var("RUST_BACKTRACE") {
        cmd.env("RUST_BACKTRACE", x);
    }
    if let Ok(x) = env::var("VAGGA_DEBUG_CMDENV") {
        cmd.env("VAGGA_DEBUG_CMDENV", x);
    }

    let build_start = Instant::now();
    let result = cmd.status();
    unmount(&Path::new("/vagga/root"))?;
    remove_dir(&Path::new("/vagga/root"))
        .map_err(|e| format!("Can't unlink root: {}", e))?;
    unmount(&Path::new("/vagga/container"))?;
    remove_dir(&Path::new("/vagga/container"))
        .map_err(|e| format!("Can't unlink root: {}", e))?;
    match result {
        Ok(s) if s.success() => {}
        Ok(s) => return Err(format!("Builder {}", s)),
        Err(e) => return Err(format!("Error running builder: {}", e)),
    };

    let ver = if let Some(ver) = ver { ver }
        else {
            match get_version_hash(container, wrapper) {
                Ok(Some(ver)) => {
                    if ver.len() == 64 && ver[..].is_ascii() {
                        ver
                    } else {
                        return Err(format!("Internal Error: \
                                Wrong version returned: {:?}", ver));
                    }
                }
                Ok(None) => {
                    return Err(format!("Internal Error: \
                            Can't version even after build"));
                },
                Err(e) => return Err(e),
            }
        };
    let name = format!("{}.{}", container,
        &ver[..8]);
    let finalpath = Path::new("/vagga/base/.roots").join(&name);
    debug!("Committing {:?} -> {:?}", tmppath, finalpath);
    match commit_root(&tmppath, &finalpath) {
        Ok(()) => {}
        Err(x) => {
            return Err(format!("Error committing root dir: {}", x));
        }
    }
    let duration = build_start.elapsed();
    warn!("Container {} ({}) built in {} seconds.",
        container, &ver[..8], duration.as_secs());
    return Ok(name);
}

pub fn build_container_cmd(wrapper: &Wrapper, cmdline: Vec<String>)
    -> Result<i32, String>
{
    let mut name: String = "".to_string();
    let mut force: bool = false;
    let mut no_image: bool = false;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("
            Internal vagga tool to setup basic system sandbox
            ");
        ap.refer(&mut name)
            .add_argument("container_name", Store,
                "Container name to build");
        ap.refer(&mut force)
            .add_option(&["--force"], StoreTrue,
                "Force build even if container is considered up to date");
        ap.refer(&mut no_image)
            .add_option(&["--no-image-download"], StoreTrue,
                "Do not download image");
        match ap.parse(cmdline, &mut stdout(), &mut stderr()) {
            Ok(()) => {}
            Err(0) => return Ok(0),
            Err(_) => {
                return Ok(122);
            }
        }
    }
    setup::setup_base_filesystem(
        wrapper.project_root, wrapper.ext_settings)?;

    build_wrapper(&name, force, no_image, wrapper)
    .map(|x| unsafe { File::from_raw_fd(3) }.write_all(x.as_bytes()).unwrap())
    .map(|_| 0)
}

pub fn build_wrapper(name: &str, force: bool, no_image: bool, wrapper: &Wrapper)
    -> Result<String, String>
{
    let container = wrapper.config.containers.get(name)
        .ok_or(format!("Container {:?} not found", name))?;
    for &Step(ref step) in container.setup.iter() {
        if let Some(name) = step.is_dependent_on() {
            build_wrapper(name, force, no_image, wrapper)
                .map(|x| debug!("Built container with name {}", x))
                .map(|()| 0)?;
        }
    }

    return build_container(name, force, no_image, wrapper)
}

pub fn print_version_hash_cmd(wrapper: &Wrapper, cmdline: Vec<String>)
    -> Result<i32, String>
{
    let opt = match Options::parse(&cmdline, true) {
        Ok(x) => x,
        Err(e) => return Ok(e),
    };
    setup::setup_base_filesystem(
        wrapper.project_root, wrapper.ext_settings)?;
    if let Some(hash) = _get_version_hash(&opt, wrapper)? {
        let res = if opt.short { &hash[..8] } else { &hash[..] };
        if opt.fd3 {
            unsafe { File::from_raw_fd(3) }.write_all(res.as_bytes())
                .map_err(|e| format!("Error writing to fd 3: {}", e))?;
        } else {
            println!("{}", res);
        }
        Ok(0)
    } else {
        Ok(29)
    }
}

