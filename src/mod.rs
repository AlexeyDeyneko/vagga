extern crate serialize;
extern crate collections;
extern crate debug;
extern crate libc;
extern crate green;
extern crate rustuv;

extern crate quire;
extern crate argparse;

use argparse::{ArgumentParser, StoreOption, List};
use std::os::{getcwd, args};
use std::os::set_exit_status;
use std::io::stdio::stderr;

use self::config::find_config;
use self::build::{BuildTask, build_container};
use self::run::{RunTask, run_chroot, run_container};
use self::env::{Environ};


mod config;
mod build;
mod run;
mod env;

fn _main() -> int {
    let mut err = stderr();
    let workdir = getcwd();

    let vcmd = args().move_iter().next().unwrap();
    let mypath = Path::new(vcmd.as_slice());
    let env = Environ {
        vagga_dir: mypath.dir_path(),
        vagga_path: mypath,
        vagga_command: vcmd.clone(),
    };

    let (config, project_root) = match find_config(&workdir) {
        Ok(tup) => tup,
        Err(e) => {
            err.write_line(e.as_slice()).ok();
            return 2;
        }
    };

    let mut cmd: Option<String> = None;
    let mut args: Vec<String> = Vec::new();
    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut cmd)
          .add_argument("command", box StoreOption::<String>,
                "A vagga command to run");
        ap.refer(&mut args)
          .add_argument("args", box List::<String>,
                "Arguments for the command");
        match ap.parse_args() {
            Ok(()) => {}
            Err(x) => return x,
        }
    }

    if cmd.is_none() {
        err.write_line("Available commands:").ok();
        for (k, _) in config.commands.iter() {
            err.write_str("    ").ok();
            err.write_line(k.as_slice()).ok();
        }
        return 1;
    }
    match cmd.unwrap().as_slice() {
        "_build" => {
            if args.len() != 1 {
                err.write_line("Usage:\n    vagga _build container_name").ok();
                return 1;
            }
            let name = args.get(0);
            match build_container(BuildTask {
                environ: &env,
                config: &config,
                name: name,
                work_dir: &workdir,
                project_root: &project_root,
                stderr: &mut err,
                })
            {
                Ok(()) => {}
                Err(text) =>  {
                    err.write_line(text.as_slice()).ok();
                    return 2;
                }
            }
        }
        "_run" => {
            if args.len() < 2 {
                err.write_line(
                    "Usage:\n    vagga _run container_name command [args]"
                ).ok();
                return 1;
            }
            let name = args.get(0);
            match run_container(RunTask {
                environ: &env,
                config: &config,
                container: name,
                command: args.slice(1, args.len()),
                work_dir: &workdir,
                project_root: &project_root,
                stderr: &mut err,
                })
            {
                Ok(x) => {
                    return x;
                }
                Err(text) =>  {
                    err.write_line(text.as_slice()).ok();
                    return 2;
                }
            }
        }
        "__chroot_and_run" => {
            if args.len() < 2 {
                err.write_line(
                    "Usage:\n    vagga __chroot_and_run container_name command [args]"
                ).ok();
                return 1;
            }
            let name = args.get(0);
            match run_chroot(RunTask {
                environ: &env,
                config: &config,
                container: name,
                command: args.slice(1, args.len()),
                work_dir: &workdir,
                project_root: &project_root,
                stderr: &mut err,
                })
            {
                Ok(x) => {
                    return x;
                }
                Err(text) =>  {
                    err.write_line(text.as_slice()).ok();
                    return 2;
                }
            }
        }
        x => {
            err.write_line(format!("Unknown command {}", x).as_slice()).ok();
            return 1;
        }
    }

    return 0;
}

fn main() {
    set_exit_status(_main());
}
