use std::cmp::min;
use std::env;
use std::fs::{File};
use std::io::{BufReader, BufRead};
use std::str::FromStr;
use std::str::from_utf8;
use std::path::{Path, PathBuf};
use unshare::{Command, Stdio};

use libc::{geteuid, getegid};
use libc::{uid_t, gid_t};

use config::Range;
use config::Settings;
use self::Uidmap::*;
use process_util::{env_path_find, capture_stdout};

#[derive(Clone)]
pub enum Uidmap {
    Singleton(uid_t, gid_t),
    Ranges(Vec<(uid_t, uid_t, uid_t)>, Vec<(gid_t, gid_t, gid_t)>),
}


fn read_uid_map(file_path: &str, username: &str) -> Result<Vec<Range>,String> {
    let file = try_msg!(File::open(&Path::new(file_path)),
        "Can't open {path}: {err}", path=file_path);
    let mut res = Vec::new();
    let reader = BufReader::new(file);
    for (num, line) in reader.lines().enumerate() {
        let line = try_msg!(line, "Error reading {path}: {err}", path=file_path);
        let parts: Vec<&str> = line[..].split(':').collect();
        if parts.len() == 0 || parts[0].trim().starts_with('#') {
            continue;
        }
        if parts.len() != 3 {
            return Err(format!("{}:{}: Bad syntax: {:?}",
                file_path, num+1, line));
        }
        let start = FromStr::from_str(parts[1]);
        let count: Result<uid_t, _> = FromStr::from_str(parts[2].trim_right());
        if parts.len() != 3 || start.is_err() || count.is_err() {
            return Err(format!("{}:{}: Bad syntax: {:?}",
                file_path, num+1, line));
        }
        if parts[0].eq(username) {
            let start: uid_t = start.unwrap();
            let end = start + count.unwrap() - 1;
            res.push(Range::new(start, end));
        }
    }
    return Ok(res);
}

pub fn match_ranges(req: &Vec<Range>, allowed: &Vec<Range>, own_id: uid_t)
    -> Result<Vec<(uid_t, uid_t, uid_t)>, ()>
{
    let mut res = vec!((0, own_id, 1));
    let mut reqiter = req.iter();
    let mut reqval = *reqiter.next().unwrap();
    let mut allowiter = allowed.iter();
    let mut allowval = *allowiter.next().unwrap();
    loop {
        if reqval.start() == 0 {
            reqval = reqval.shift(1);
        }
        if allowval.start() == 0 {
            allowval = allowval.shift(1);
        }
        let clen = min(reqval.len(), allowval.len());
        if clen > 0 {
            res.push((reqval.start(), allowval.start(), clen));
        }
        reqval = reqval.shift(clen);
        allowval = allowval.shift(clen);
        if reqval.len() == 0 {
            reqval = match reqiter.next() {
                Some(val) => *val,
                None => break,
            };
        }
        if allowval.len() == 0 {
            allowval = match allowiter.next() {
                Some(val) => *val,
                None => return Err(()),
            };
        }
    }
    return Ok(res);
}

pub fn get_max_uidmap() -> Result<Uidmap, String>
{
    let mut cmd = Command::new(env_path_find("id")
                               .unwrap_or(PathBuf::from("/usr/bin/id")));
    cmd.arg("-u").arg("-n");
    if let Ok(path) = env::var("_VAGGA_PATH") {
        cmd.env("PATH", path);
    }
    cmd.stdin(Stdio::null()).stderr(Stdio::inherit());
    let username = try!(capture_stdout(cmd)
        .map_err(|e| format!("Error running `id -u -n`: {}", e))
        .and_then(|val| from_utf8(&val).map(|x| x.trim().to_string())
                   .map_err(|e| format!("Can't decode username: {}", e))));
    let uid_map = read_uid_map("/etc/subuid", &username)
        .map_err(|e| error!("Error reading uidmap: {}", e));
    let gid_map = read_uid_map("/etc/subgid", &username)
        .map_err(|e| error!("Error reading gidmap: {}", e));

    let uid = unsafe { geteuid() };
    let gid = unsafe { getegid() };
    if let (Ok(uid_map), Ok(gid_map)) = (uid_map, gid_map) {
        if uid_map.len() == 0 && gid_map.len() == 0 && uid == 0 {
            let uid_rng = try!(read_uid_ranges("/proc/self/uid_map", true));
            let gid_rng = try!(read_uid_ranges("/proc/self/gid_map", true));
            return Ok(Ranges(
                uid_rng.into_iter()
                    .map(|r| (r.start(), r.start(), r.len())).collect(),
                gid_rng.into_iter()
                    .map(|r| (r.start(), r.start(), r.len())).collect()));
        }
        let mut uids = vec!((0, uid, 1));
        for &rng in uid_map.iter() {
            let mut rng = rng;
            if uid >= rng.start() && uid <= rng.end() {
                // TODO(tailhook) implement better heuristic
                assert!(uid == rng.start());
                rng = rng.shift(1);
                if rng.len() == 0 { continue; }
            }
            uids.push((rng.start(), rng.start(), rng.len()));
        }

        let mut gids = vec!((0, gid, 1));
        for &rng in gid_map.iter() {
            let mut rng = rng;
            if gid >= rng.start() && gid <= rng.end() {
                // TODO(tailhook) implement better heuristic
                assert!(gid == rng.start());
                rng = rng.shift(1);
                if rng.len() == 0 { continue; }
            }
            gids.push((rng.start(), rng.start(), rng.len()));
        }

        return Ok(Ranges(uids, gids));
    } else {
        warn!("Could not read /etc/subuid or /etc/subgid \
            (see http://bit.ly/err_subuid)");
        return Ok(Singleton(uid, gid));
    }
}

fn read_uid_ranges(path: &str, read_inside: bool) -> Result<Vec<Range>, String>
{
    let file = BufReader::new(try!(File::open(&Path::new(path))
        .map_err(|e| format!("Error reading uid/gid map: {}", e))));
    let mut result = vec!();
    for line in file.lines() {
        let line = try!(line
            .map_err(|_| format!("Error reading uid/gid map")));
        let mut words = line[..].split_whitespace();
        let inside: u32 = try!(words.next().and_then(|x| FromStr::from_str(x).ok())
            .ok_or(format!("uid/gid map format error")));
        let outside: u32 = try!(words.next().and_then(|x| FromStr::from_str(x).ok())
            .ok_or(format!("uid/gid map format error")));
        let count: u32 = try!(words.next().and_then(|x| FromStr::from_str(x).ok())
            .ok_or(format!("uid/gid map format error")));
        if read_inside {
            result.push(Range::new(inside, inside+count-1));
        } else {
            result.push(Range::new(outside, outside+count-1));
        }
    }
    return Ok(result);
}

pub fn map_users(settings: &Settings, uids: &Vec<Range>, gids: &Vec<Range>)
    -> Result<Uidmap, String>
{
    let default_uids = vec!(Range::new(0, 0));
    let default_gids = vec!(Range::new(0, 0));
    let uids = if uids.len() > 0 { uids } else { &default_uids };
    let gids = if gids.len() > 0 { gids } else { &default_gids };
    match settings.uid_map {
        None => {
            let ranges = try!(read_uid_ranges("/proc/self/uid_map", true));
            let uid_map = try!(match_ranges(uids, &ranges, 0)
                .map_err(|()| {
                    return format!("Number of allowed subuids is too small. \
                        Required {:?}, allowed {:?}. You either need to increase \
                        allowed numbers in /etc/subuid (preferred) or decrease \
                        needed ranges in vagga.yaml by adding `uids` key \
                        to container config", uids, ranges);
                }));
            let ranges = try!(read_uid_ranges("/proc/self/gid_map", true));
            let gid_map = try!(match_ranges(gids, &ranges, 0)
                .map_err(|()| {
                    return format!("Number of allowed subgids is too small. \
                        Required {:?}, allowed {:?}. You either need to increase \
                        allowed numbers in /etc/subgid (preferred) or decrease \
                        needed ranges in vagga.yaml by adding `gids` key \
                        to container config", gids, ranges);
                }));
            Ok(Ranges(uid_map, gid_map))
        },
        Some((ref uids, ref gids)) => {
            Ok(Ranges(uids.clone(), gids.clone()))
        }
    }
}
