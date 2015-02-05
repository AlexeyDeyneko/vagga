use std::io::stdio::{stdout, stderr};
use std::io::fs::{readdir, rmdir_recursive};

use argparse::{ArgumentParser, PushConst, StoreTrue};

use super::setup;
use super::Wrapper;

#[derive(Copy)]
enum Action {
    Temporary,
    Old,
    Everything,
    Orphans,
}


pub fn clean_cmd(wrapper: &Wrapper, cmdline: Vec<String>)
    -> Result<isize, String>
{
    let mut global = false;
    let mut dry_run = false;
    let mut actions = vec!();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("
            Performs various cleanup tasks
            ");
        ap.refer(&mut actions)
          .add_option(&["--tmp", "--tmp-folders"],
                Box::new(PushConst(Action::Temporary)),
                "Clean temporary containers (failed builds)")
          .add_option(&["--old", "--old-containers"],
                Box::new(PushConst(Action::Old)),
                "Clean old versions of containers (configurable)")
          .add_option(&["--everything"],
                Box::new(PushConst(Action::Everything)),
                "Clean whole `.vagga` folder. Useful when deleting a project.
                 With ``--global`` cleans whole storage-dir and cache-dir")
          .add_option(&["--orphans"],
                Box::new(PushConst(Action::Orphans)),
                "Without `--global` removes containers which are not in
                 vagga.yaml any more. With `--global` removes all folders
                 which have `.lnk` pointing to nowhere (i.e. project dir
                 already deleted while vagga folder is not)")
          .required();
        ap.refer(&mut global)
          .add_option(&["--global"], Box::new(StoreTrue),
                "Apply cleanup command to all containers. Works only \
                if `storage-dir` is configured in settings");
        ap.refer(&mut dry_run)
          .add_option(&["-n", "--dry-run"], Box::new(StoreTrue),
                "Dry run. Don't delete everything, just print");
        match ap.parse(cmdline, &mut stdout(), &mut stderr()) {
            Ok(()) => {}
            Err(0) => return Ok(0),
            Err(_) => {
                return Ok(1);
            }
        }
    }
    if global && wrapper.ext_settings.storage_dir.is_none() {
        error!("The --global flag is only meaningful if you configure \
            storage-dir in settings");
        return Ok(2);
    }
    for action in actions.iter() {
        let res = match *action {
            Action::Temporary => clean_temporary(wrapper, global, dry_run),
            _ => unimplemented!(),
        };
        match res {
            Ok(()) => {}
            Err(err) => {
                error!("Error cleaning up: {}", err);
                return Ok(3);
            }
        }
    }
    return Ok(0);
}

fn clean_temporary(wrapper: &Wrapper, global: bool, dry_run: bool)
    -> Result<(), String>
{
    if global {
        panic!("Global cleanup is not implemented yet");
    }
    let base = match try!(setup::get_vagga_base(
        wrapper.project_root, wrapper.ext_settings))
    {
        Some(base) => base,
        None => {
            warn!("No vagga directory exists");
            return Ok(());
        }
    };
    let roots = base.join(".roots");
    for path in try!(readdir(&roots)
            .map_err(|e| format!("Can't read dir {:?}: {}", roots, e)))
            .iter()
    {
        if path.filename_str().map(|n| n.starts_with(".tmp")).unwrap_or(false)
        {
            if dry_run {
                println!("Would remove {:?}", path);
            } else {
                try!(rmdir_recursive(path)
                     .map_err(|x| format!("Error removing directory: {}", x)));
            }
        }
    }

    return Ok(());
}
