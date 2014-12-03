#![feature(phase, if_let)]

extern crate quire;
extern crate argparse;
extern crate serialize;
extern crate regex;
#[phase(plugin)] extern crate regex_macros;

extern crate config;
#[phase(plugin, link)] extern crate container;

use std::rc::Rc;
use std::io::stderr;
use std::io::ALL_PERMISSIONS;
use std::io::{TypeSymlink, TypeDirectory, FileNotFound};
use std::os::{getcwd, set_exit_status, self_exe_path, getenv};
use std::io::fs::{mkdir, copy, readlink, symlink};
use std::io::fs::PathExtensions;
use std::cell::Cell;

use config::find_config;
use container::signal;
use container::monitor::{Monitor, Executor};
use container::container::{Command};
use container::mount::{mount_tmpfs, bind_mount, unmount};
use container::mount::{mount_ro_recursive, mount_pseudo};
use container::root::change_root;
use settings::{read_settings, MergedSettings};
use argparse::{ArgumentParser, Store, List};

mod settings;


struct RunBuilder {
    cmd: String,
    container: String,
    result: Cell<int>,
}


impl Executor for RunBuilder {
    fn command(&self) -> Command {
        let mut cmd = Command::new(self.cmd.clone(),
            Path::new("/vagga/bin").join(self.cmd.as_slice()));
        cmd.arg(self.container.as_slice());
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
    fn finish(&self, status: int) -> bool {
        self.result.set(status);
        return false;
    }
}



fn safe_ensure_dir(dir: &Path) -> Result<(), String> {
    match dir.lstat() {
        Ok(stat) if stat.kind == TypeSymlink => {
            return Err(format!(concat!("The `{0}` dir can't be a symlink. ",
                               "Please run `unlink {0}`"), dir.display()));
        }
        Ok(stat) if stat.kind == TypeDirectory => {
            // ok
        }
        Ok(_) => {
            return Err(format!(concat!("The `{0}` must be a directory. ",
                               "Please run `unlink {0}`"), dir.display()));
        }
        Err(ref e) if e.kind == FileNotFound => {
            try!(mkdir(dir, ALL_PERMISSIONS)
                .map_err(|e| format!("Can't create `{}`: {}",
                                     dir.display(), e)));
        }
        Err(ref e) => {
            return Err(format!("Can't stat `{}`: {}", dir.display(), e));
        }
    }
    return Ok(());
}

fn make_mountpoint(project_root: &Path) -> Result<(), String> {
    let vagga_dir = project_root.join(".vagga");
    try!(safe_ensure_dir(&vagga_dir));
    let mnt_dir = vagga_dir.join(".mnt");
    try!(safe_ensure_dir(&mnt_dir));
    return Ok(());
}

fn create_storage_dir(storage_dir: &Path, project_root: &Path)
    -> Result<Path, String>
{
    let name = match project_root.filename_str() {
        Some(name) => name,
        None => return Err(format!(
            "Project dir `{}` is either root or has bad characters",
            project_root.display())),
    };
    let path = storage_dir.join(name);
    if !path.exists() {
        return Ok(path);
    }
    for i in range(1i, 101i) {
        let result = format!("{}-{}", name, i);
        let path = storage_dir.join(result);
        if !path.exists() {
            return Ok(path);
        }
    }
    return Err(format!("Too many similar paths named {} in {}",
        name, storage_dir.display()));
}

fn vagga_base(project_root: &Path, settings: &MergedSettings)
    -> Result<Path, String>
{
    if let Some(ref dir) = settings.storage_dir {
        let lnkdir = project_root.join(".vagga/.lnk");
        match readlink(&lnkdir) {
            Ok(lnk) => {
                if let Some(name) = lnk.filename() {
                    let target = dir.join(name);
                    if Path::new(lnk.dirname()) != *dir {
                        return Err(concat!("You have set storage_dir to {}, ",
                            "but .vagga/.lnk points to {}. You probably need ",
                            "to run `ln -sfn {} .vagga/.lnk`").to_string());
                    }
                    if !lnkdir.exists() {
                        return Err(concat!("Your .vagga/.lnk points to a ",
                            "non-existent directory. Presumably you deleted ",
                            "dir {}. Just remove .vagga/.lnk now."
                            ).to_string());
                    }
                    return Ok(target);
                } else {
                    return Err(format!(concat!("Bad link .vagga/.lnk: {}.",
                        " You are pobably need to remove it now"),
                        lnk.display()));
                }
            }
            Err(ref e) if e.kind == FileNotFound => {
                let target = try!(create_storage_dir(dir, project_root));
                try!(safe_ensure_dir(&target));
                try_str!(symlink(&target, &lnkdir));
                return Ok(target)
            }
            Err(ref e) => {
                return Err(format!("Can't read link .vagga/.lnk: {}", e));
            }
        };
    } else {
        return Ok(project_root.join(".vagga"));
    }
}

fn make_cache_dir(project_root: &Path, vagga_base: &Path,
    settings: &MergedSettings)
    -> Result<Path, String>
{
    match settings.cache_dir {
        Some(ref dir) if settings.shared_cache => {
            if !dir.exists() {
                return Err(format!(concat!("Cache directory `{}` must exists.",
                    " Please either create it or remove that configuration",
                    " setting"), dir.display()));
            }
            return Ok(dir.clone());
        }
        _ => {
            let dir = vagga_base.join(".cache");
            try!(safe_ensure_dir(&dir));
            return Ok(dir);
        }
   }
}


fn setup_filesystem(project_root: &Path, settings: &MergedSettings)
    -> Result<(), String>
{
    let mnt_dir = project_root.join(".vagga/.mnt");
    try!(make_mountpoint(project_root));
    try!(mount_tmpfs(&mnt_dir, "size=10m"));

    let proc_dir = mnt_dir.join("proc");
    try_str!(mkdir(&proc_dir, ALL_PERMISSIONS));
    try!(mount_pseudo(&proc_dir, "proc", "", false));

    let dev_dir = mnt_dir.join("dev");
    try_str!(mkdir(&dev_dir, ALL_PERMISSIONS));
    try!(bind_mount(&Path::new("/dev"), &dev_dir));

    let vagga_dir = mnt_dir.join("vagga");
    try_str!(mkdir(&vagga_dir, ALL_PERMISSIONS));

    let bin_dir = vagga_dir.join("bin");
    try_str!(mkdir(&bin_dir, ALL_PERMISSIONS));
    try!(bind_mount(&self_exe_path().unwrap(), &bin_dir));
    try!(mount_ro_recursive(&bin_dir));

    let etc_dir = mnt_dir.join("etc");
    try_str!(mkdir(&etc_dir, ALL_PERMISSIONS));
    try!(copy(&Path::new("/etc/hosts"), &etc_dir.join("hosts"))
        .map_err(|e| format!("Error copying /etc/hosts: {}", e)));
    try!(copy(&Path::new("/etc/resolv.conf"), &etc_dir.join("resolv.conf"))
        .map_err(|e| format!("Error copying /etc/resolv.conf: {}", e)));

    let roots_dir = vagga_dir.join("roots");
    try_str!(mkdir(&roots_dir, ALL_PERMISSIONS));
    let vagga_base = try!(vagga_base(project_root, settings));
    let local_roots = vagga_base.join(".roots");
    try!(safe_ensure_dir(&local_roots));
    try!(bind_mount(&local_roots, &roots_dir));

    let cache_dir = vagga_dir.join("cache");
    try_str!(mkdir(&cache_dir, ALL_PERMISSIONS));
    let locl_cache = try!(make_cache_dir(project_root, &vagga_base, settings));
    try!(bind_mount(&locl_cache, &cache_dir));

    let work_dir = mnt_dir.join("work");
    try_str!(mkdir(&work_dir, ALL_PERMISSIONS));
    try!(bind_mount(project_root, &work_dir));


    let old_root = vagga_dir.join("old_root");
    try_str!(mkdir(&old_root, ALL_PERMISSIONS));
    try!(change_root(&mnt_dir, &old_root));
    try!(unmount(&Path::new("/vagga/old_root")));

    return Ok(());
}


pub fn run() -> int {
    let mut err = stderr();
    let mut cmd: String = "".to_string();
    let mut args: Vec<String> = Vec::new();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("
            Internal vagga tool to setup basic system sandbox
            ");
        ap.refer(&mut cmd)
          .add_argument("command", box Store::<String>,
                "A vagga command to run")
          .required();
        ap.refer(&mut args)
          .add_argument("args", box List::<String>,
                "Arguments for the command");
        ap.stop_on_first_argument(true);
        match ap.parse_args() {
            Ok(()) => {}
            Err(0) => return 0,
            Err(_) => return 122,
        }
    }

    let workdir = getcwd();

    let (config, project_root) = match find_config(&workdir) {
        Ok(tup) => tup,
        Err(e) => {
            err.write_line(e.as_slice()).ok();
            return 126;
        }
    };
    let (ext_settings, int_settings) = match read_settings(&project_root)
    {
        Ok(tup) => tup,
        Err(e) => {
            err.write_line(e.as_slice()).ok();
            return 126;
        }
    };

    if let Err(text) = setup_filesystem(&project_root, &ext_settings) {
        err.write_line(text.as_slice()).ok();
        return 122;
    }

    let result = Cell::new(-1);
    let mut mon = Monitor::new();
    if cmd.as_slice() == "_build" {
        mon.add(Rc::new("version".to_string()), box RunBuilder {
            cmd: "vagga_version".to_string(),
            container: args[0].to_string(),
            result: result,
        });
    } else {
        unimplemented!();
    }
    mon.run();
    println!("STATUS {}", result.get());
    return result.get();
}

fn main() {
    signal::block_all();
    let val = run();
    set_exit_status(val);
}
