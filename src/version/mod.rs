use std::default::Default;
use std::fs::File;
use std::path::Path;
use std::process::exit;
use std::io::{Write};
use std::os::unix::io::FromRawFd;

use argparse::{ArgumentParser, Store};
use shaman::digest::Digest;

use config::read_config;
use config::Settings;


mod version;


pub fn run() -> i32 {
    let mut container: String = "".to_string();
    let mut settings: Settings = Default::default();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("
            A tool which versions containers
            ");
        ap.refer(&mut container)
          .add_argument("container", Store,
                "A container to version")
          .required();
        ap.refer(&mut settings)
          .add_option(&["--settings"], Store,
                "User settings for the container build");
        match ap.parse_args() {
            Ok(()) => {}
            Err(0) => return 0,
            Err(_) => return 122,
        }
    }

    // TODO(tailhook) read also config from /work/.vagga/vagga.yaml
    let cfg = read_config(&Path::new("/work/vagga.yaml")).ok()
        .expect("Error parsing configuration file");  // TODO
    let cont = cfg.containers.get(&container)
        .expect("Container not found");  // TODO

    let mut hash = match version::all(&cont.setup[..], &cfg) {
        Ok(hash) => hash,
        Err((cmd, e)) => {
            error!("Error versioning command {}: {}", cmd, e);
            return 1;
        }
    };

    debug!("Got hash {:?}", hash.result_str());
    match unsafe { File::from_raw_fd(3) }.write_all(hash.result_str().as_bytes()) {
        Ok(()) => {}
        Err(e) => {
            error!("Error writing hash: {}", e);
            return 1;
        }
    }
    return 0;
}

pub fn main() {
    let val = run();
    exit(val);
}
