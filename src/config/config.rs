use std::default::Default;
use std::path::{PathBuf, Path};
use std::fs::PathExt;

use std::collections::BTreeMap;
use rustc_serialize::{Decoder};

use quire::parse_config;
use quire::validate as V;
use super::containers;
use super::containers::Container;
use super::command::{MainCommand, command_validator};
use super::range::Range;

#[derive(RustcDecodable)]
pub struct Config {
    pub commands: BTreeMap<String, MainCommand>,
    pub containers: BTreeMap<String, Container>,
}

pub fn config_validator<'a>() -> Box<V::Validator + 'a> {
    return Box::new(V::Structure { members: vec!(
        ("containers".to_string(), Box::new(V::Mapping {
            key_element: Box::new(V::Scalar {
                .. Default::default()}) as Box<V::Validator>,
            value_element: containers::container_validator(),
            .. Default::default()}) as Box<V::Validator>),
        ("commands".to_string(), Box::new(V::Mapping {
            key_element: Box::new(V::Scalar {
                .. Default::default()}) as Box<V::Validator>,
            value_element: command_validator(),
            .. Default::default()}) as Box<V::Validator>),
    ), .. Default::default()}) as Box<V::Validator>;
}

fn find_config_path(work_dir: &PathBuf) -> Option<(PathBuf, PathBuf)> {
    let mut dir = work_dir.clone();
    loop {
        let fname = dir.join(".vagga/vagga.yaml");
        if fname.exists() {
            return Some((dir, fname));
        }
        let fname = dir.join("vagga.yaml");
        if fname.exists() {
            return Some((dir, fname));
        }
        if !dir.pop() {
            return None;
        }
    }
}

pub fn find_config(work_dir: &PathBuf) -> Result<(Config, PathBuf), String> {
    let (cfg_dir, filename) = match find_config_path(work_dir) {
        Some(pair) => pair,
        None => return Err(format!(
            "Config not found in path {}", work_dir.display())),
    };
    assert!(cfg_dir.is_absolute());
    return Ok((try!(read_config(&filename)), cfg_dir));
}

pub fn read_config(filename: &Path) -> Result<Config, String> {
    let mut config: Config = match parse_config(
        filename, &*config_validator(), Default::default())
    {
        Ok(cfg) => cfg,
        Err(e) => {
            return Err(format!("Config {} cannot be read: {}",
                filename.display(), e));
        }
    };
    for (_, ref mut container) in config.containers.iter_mut() {
        if container.uids.len() == 0 {
            container.uids.push(Range::new(0, 65535));
        }
        if container.gids.len() == 0 {
            container.gids.push(Range::new(0, 65535));
        }
    }
    return Ok(config);
}
