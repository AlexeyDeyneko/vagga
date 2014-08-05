use std::io;
use std::os::getenv;
use std::path::BytesContainer;
use std::str::from_utf8_lossy;
use get_environ = std::os::env;
use std::io::fs::{mkdir, rmdir_recursive, rename, symlink, unlink, readlink};
use std::io::process::{ExitStatus, Command, Ignored, InheritFd};
use std::io::stdio::{stdout, stderr};

use super::sha256::Sha256;
use super::sha256::Digest;

use argparse::{ArgumentParser, Store, StoreTrue};

use super::env::{Environ, Container};
use super::options::env_options;
use super::linux::ensure_dir;


fn makedirs(path: &Path) -> Result<(),String> {
    if path.exists() {
        return Ok(());
    }
    try!(makedirs(&path.dir_path()));
    return match mkdir(path, io::UserRWX) {
        Ok(()) => Ok(()),
        Err(e) => Err(format!("Can't mkdir: {}", e)),
    };
}

pub fn build_command(environ: &mut Environ, args: Vec<String>)
    -> Result<int, String>
{
    let mut cname = "devel".to_string();
    let mut force = false;
    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut cname)
            .add_argument("container", box Store::<String>,
                "A name of the container to build")
            .required();
        ap.refer(&mut force)
            .add_option(["--force"], box StoreTrue,
                "Force rebuild of container event if it already exists");
        env_options(environ, &mut ap);
        match ap.parse(args, &mut stdout(), &mut stderr()) {
            Ok(()) => {}
            Err(0) => return Ok(0),
            Err(_) => return Ok(122),
        }
    }
    let mut container = try!(environ.get_container(&cname));
    try!(build_container(environ, &mut container, force));
    try!(link_container(environ, &container));

    return Ok(0);
}

pub fn link_container(environ: &Environ, container: &Container)
    -> Result<(), String>
{
    let container_root = container.container_root.as_ref().unwrap();
    let cdir = container_root.filename().unwrap();
    let tmptarget = environ.local_vagga.join(
        (".lnk.".container_into_owned_bytes() + cdir).as_slice());
    if tmptarget.exists() {
        match unlink(&tmptarget) {
            Ok(()) => {}
            Err(x) => return Err(format!("Error removing file: {}", x)),
        }
    }
    let target = environ.local_vagga.join(container.name.as_slice());
    debug!("Linking {} -> {}", container_root.display(), target.display())
    let relative_root = Path::new(".roots").join(cdir);
    match symlink(&relative_root, &tmptarget)
          .and(rename(&tmptarget, &target)) {
        Ok(()) => {}
        Err(x) => return Err(format!("Can't symlink new root: {}", x)),
    }
    if !container.name.eq(&container.fullname) {
        let target = environ.local_vagga.join(container.fullname.as_slice());
        debug!("Linking {} -> {}", container_root.display(), target.display())
        match symlink(&relative_root, &tmptarget)
              .and(rename(&tmptarget, &target)) {
            Ok(()) => {}
            Err(x) => return Err(format!("Can't symlink new root: {}", x)),
        }
    }
    return Ok(());
}

pub fn build_container(environ: &Environ, container: &mut Container,
                       force: bool)
    -> Result <bool, String>
{
    info!("Checking {}", container.name);

    let builder = &container.builder;
    let bldpath = match environ.find_builder(builder) {
        Some(b) => b,
        None => return Err(format!("Can't find builder {}, \
            check your VAGGA_PATH", builder)),
    };
    info!("Builder full path is {}", bldpath.display());
    let cache_dir = environ.local_vagga.join_many(
        [".cache", builder.as_slice()]);

    // TODO(tailhook) which path should be here?
    let path = getenv("PATH").unwrap_or(
        "/sbin:/bin:/usr/sbin:/usr/bin:/usr/local/bin:/usr/local/sbin"
        .to_string());

    let mut env: Vec<(&[u8], &[u8])> = Vec::new();
    let mut caller_env: Vec<(String, String)> = Vec::new();
    let mut parameters: Vec<(String, String)> = Vec::new();
    env.push(("HOME".as_bytes(), "/homeless-shelter".as_bytes()));
    env.push(("PATH".as_bytes(), path.as_bytes()));
    env.push(("container_name".as_bytes(), container.name.as_bytes()));
    env.push(("container_fullname".as_bytes(), container.fullname.as_bytes()));
    env.push(("cache_dir".as_bytes(), cache_dir.as_vec()));
    env.push(("project_root".as_bytes(), environ.project_root.as_vec()));
    for (k, v) in get_environ().move_iter() {
        let pk = "CALLER_".to_string() + k;
        let pv = v;
        caller_env.push((pk, pv));
    }
    for (k, v) in container.parameters.iter() {
        parameters.push((builder + "_" + *k, v.clone()));
    }
    for &(ref k, ref v) in caller_env.iter() {
        env.push((k.container_as_bytes(), v.container_as_bytes()));
    }
    for &(ref k, ref v) in parameters.iter() {
        env.push((k.container_as_bytes(), v.container_as_bytes()));
    }

    let version_sh = bldpath.join("version.sh");
    let mut digest = Sha256::new();
    if version_sh.exists() {
        digest.input_str(builder.as_slice());
        digest.input_str(":");
        match Command::new(&version_sh).env(env.as_slice())
            .cwd(&environ.project_root)
            .stdin(Ignored).stderr(InheritFd(2)).output() {
            Ok(out) => match out.status {
                ExitStatus(0) => {
                    debug!("Version data ```{}```",
                           from_utf8_lossy(out.output.as_slice()));
                    digest.input(out.output.as_slice());
                }
                e => {
                    return Err(format!("Error running {}: {}",
                        version_sh.display(), e));
                }
            },
            Err(e) => {
                return Err(format!("Error running {}: {}",
                    version_sh.display(), e));
            }
        }
    } else {
        warn!("This backend has no versioning. Using primitive one.");
        digest.input_str(builder.as_slice());
        digest.input_str(":");
        for (k, v) in container.parameters.iter() {
            digest.input_str(k.as_slice());
            digest.input_str("=");
            digest.input_str(v.as_slice());
            digest.input_str(";");
        }
    }
    match container.provision {
        Some(ref x) => digest.input_str(x.as_slice()),
        None => {}
    }

    let fullhash = digest.result_str();
    let hash = fullhash.as_slice().slice_to(7);

    let build_sh = bldpath.join("build.sh");
    if !build_sh.exists() {
        return Err(format!("Builder {} does not exist", builder));
    }


    let cdir = format!("{}.{}", container.fullname, hash);
    let container_root = environ.local_vagga.join_many(
        [".roots", cdir.as_slice()]);

    if container_root.exists() && !force {
        info!("Container {} already built as {}",
            container.name, container_root.display());
        container.container_root = Some(container_root);
        return Ok(false);
    }

    let artifacts_dir = environ.local_vagga.join_many(
        [".artifacts", cdir.as_slice()]);
    let container_tmp = environ.local_vagga.join_many(
        [".roots", (cdir + ".tmp").as_slice()]);

    info!("Building {} by {}", container_root.display(), build_sh.display());

    if container_tmp.exists() {
        // TODO(tailhook) join container and rm, because may have
        // files of different owners (subuid's)
        try!(rmdir_recursive(&container_tmp).map_err(
                |e| format!("Can't clean temporary root: {}", e)));
    }
    if artifacts_dir.exists() {
        // TODO(tailhook) join container and rm, because may have
        // files of different owners (subuid's)
        try!(rmdir_recursive(&artifacts_dir).map_err(
                |e| format!("Can't clean temporary root: {}", e)));
    }
    try!(makedirs(&container_tmp));

    env.push(("vagga_exe".as_bytes(), environ.vagga_exe.container_as_bytes()));
    env.push(("artifacts_dir".as_bytes(), artifacts_dir.as_vec()));
    env.push(("container_hash".as_bytes(), hash.as_bytes()));
    env.push(("container_root".as_bytes(), container_tmp.as_vec()));
    match Command::new(build_sh).env(env.as_slice())
        .cwd(&environ.project_root)
        .stdin(InheritFd(0)).stdout(InheritFd(1)).stderr(InheritFd(2))
        .status() {
        Ok(ExitStatus(0)) => {}
        Ok(x) => return Err(format!("Builder exited with status {}", x)),
        Err(x) => return Err(format!("Can't spawn: {}", x)),
    }
    if container.provision.is_some() {
        match Command::new(environ.vagga_exe.as_vec()).env(env.as_slice())
            .arg("_chroot")
            .arg("--writeable")
            .arg("--inventory")
            .arg(container_tmp.as_vec())
            .args(container.shell.as_slice())
            .arg(container.provision.as_ref().unwrap().as_slice())
            .cwd(&environ.project_root)
            .stdin(InheritFd(0)).stdout(InheritFd(1)).stderr(InheritFd(2))
            .status() {
            Ok(ExitStatus(0)) => {}
            Ok(x) => return Err(format!("Provision exited with status {}", x)),
            Err(x) => return Err(format!("Can't spawn provisor: {}", x)),
        }
    }

    for dir in ["proc", "sys", "dev", "work", "tmp", "etc"].iter() {
        try!(ensure_dir(&container_tmp.join(*dir)));
    }

    let container_old = environ.local_vagga.join_many(
        [".roots", (cdir + ".old").as_slice()]);
    if container_root.exists() {
        if container_old.exists() {
            match rmdir_recursive(&container_old) {
                Ok(()) => {}
                Err(x) => return Err(format!("Can't remove old root: {}", x)),
            }
        }
        match rename(&container_root, &container_old) {
            Ok(()) => {}
            Err(x) => return Err(format!("Can't rename old root: {}", x)),
        }
    }

    match rename(&container_tmp, &container_root) {
        Ok(()) => {}
        Err(x) => return Err(format!("Can't rename root: {}", x)),
    }

    if container_old.exists() {
        match rmdir_recursive(&container_old) {
            Ok(()) => {}
            Err(x) => return Err(format!("Can't remove old root: {}", x)),
        }
    }
    info!("Done building {} as {}",
        container.name, container_root.display());

    container.container_root = Some(container_root);

    return Ok(true);
}

pub fn ensure_container(env: &Environ, container: &mut Container)
    -> Result<(), String>
{
    if env.settings.version_check {
        try!(build_container(env, container, false));
        try!(link_container(env, container));
    } else {
        let lnk = env.local_vagga.join(container.fullname.as_slice());
        container.container_root = match readlink(&lnk) {
            Ok(path) => Some(lnk.dir_path().join(path)),
            Err(e) => return Err(format!("Container {} not found: {}",
                                         container.fullname, e)),
        };
    };
    return Ok(());
}
