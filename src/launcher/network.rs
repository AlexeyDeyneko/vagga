use std::collections::BitSet;
use std::env;
use std::fs::{remove_file};
use std::fs::File;
use std::io::{stdout, stderr, BufRead, BufReader};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::rc::Rc;
use std::str::FromStr;

use rand::thread_rng;
use rand::distributions::{Range, IndependentSample};
use libc::{pid_t};
use libc::{geteuid};
use argparse::{ArgumentParser};
use argparse::{StoreTrue, StoreFalse, List, StoreOption, Store};
use rustc_serialize::json;

use super::super::config::Config;
use super::super::container::mount::{bind_mount};
use super::super::container::nsutil::{set_namespace};
use super::super::container::signal::wait_process;
use super::super::container::container::Namespace::{NewUser, NewNet};
use super::super::container::container::Command as ContainerCommand;
use shaman::sha2::Sha256;
use shaman::digest::Digest;
use super::user;
use super::super::file_util::{create_dir, create_dir_mode};

static MAX_INTERFACES: usize = 2048;

pub struct PortForwardGuard {
    nspath: Path,
    ip: String,
    ports: Vec<u16>,
}

pub fn namespace_dir() -> Path {
    let uid = unsafe { geteuid() };
    env::var("XDG_RUNTIME_DIR")
        .map(|v| Path::new(v).join("vagga"))
        .unwrap_or(Path::new(format!("/tmp/vagga-{}", uid)))
}


pub fn create_netns(_config: &Config, mut args: Vec<String>)
    -> Result<i32, String>
{
    let interface_name = "vagga".to_string();
    let network = "172.18.255.0/30".to_string();
    let host_ip_net = "172.18.255.1/30".to_string();
    let host_ip = "172.18.255.1".to_string();
    let guest_ip = "172.18.255.2/30".to_string();
    let mut dry_run = false;
    let mut iptables = true;
    {
        args.insert(0, "vagga _create_netns".to_string());
        let mut ap = ArgumentParser::new();
        ap.set_description("
            Set's up network namespace for subsequent container runs
            ");
        ap.refer(&mut dry_run)
            .add_option(&["--dry-run"], StoreTrue,
                "Do not run commands, only show");
        ap.refer(&mut iptables)
            .add_option(&["--no-iptables"], StoreFalse,
                "Do not update iptables rules (useful you have firewall \
                 other than iptables). You need to update your firewall rules \
                 manually to have functional networking.");
        match ap.parse(args, &mut stdout(), &mut stderr()) {
            Ok(()) => {}
            Err(0) => return Ok(0),
            Err(_) => {
                return Ok(122);
            }
        }
    }

    let runtime_dir = namespace_dir();
    if !runtime_dir.exists() {
        try_msg!(create_dir_mode(&runtime_dir, 0o755),
            "Can't create runtime_dir: {err}");
    }

    let netns_file = runtime_dir.join("netns");
    let userns_file = runtime_dir.join("userns");

    if netns_file.exists() || userns_file.exists() {
        return Err("Namespaces already created".to_string());
    }

    let mut cmd = ContainerCommand::new("setup_netns".to_string(),
        env::current_exe().unwrap().parent().unwrap()
        .join("vagga_setup_netns"));
    cmd.set_max_uidmap();
    cmd.network_ns();
    cmd.set_env("TERM".to_string(),
                env::var("TERM").unwrap_or("dumb".to_string()));
    if let Ok(x) = env::var("PATH") {
        cmd.set_env("PATH".to_string(), x);
    }
    if let Ok(x) = env::var("RUST_LOG") {
        cmd.set_env("RUST_LOG".to_string(), x);
    }
    if let Ok(x) = env::var("RUST_BACKTRACE") {
        cmd.set_env("RUST_BACKTRACE".to_string(), x);
    }
    cmd.arg("gateway");
    cmd.arg("--guest-ip");
    cmd.arg(guest_ip.as_slice());
    cmd.arg("--gateway-ip");
    cmd.arg(host_ip.as_slice());
    cmd.arg("--network");
    cmd.arg(network.as_slice());
    let child_pid = if dry_run { 123456 } else { try!(cmd.spawn()) };

    println!("We will run network setup commands with sudo.");
    println!("You may need to enter your password.");

    let mut commands = vec!();

    // If we are root we may skip sudo
    let mut ip_cmd = Command::new("sudo");
    ip_cmd.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    ip_cmd.arg("ip");

    // If we are root we may skip sudo
    let mut sysctl = Command::new("sudo");
    sysctl.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    sysctl.arg("sysctl");

    let mut cmd = ip_cmd.clone();
    cmd.args(&["link", "add", "vagga_guest", "type", "veth",
              "peer", "name", interface_name.as_slice()]);
    commands.push(cmd);

    let mut cmd = ip_cmd.clone();
    cmd.args(&["link", "set", "vagga_guest", "netns"]);
    cmd.arg(format!("{}", child_pid));
    commands.push(cmd);

    let mut cmd = ip_cmd.clone();
    cmd.args(&["addr", "add", host_ip_net.as_slice(),
              "dev", interface_name.as_slice()]);
    commands.push(cmd);

    let nforward = try!(File::open(&Path::new("/proc/sys/net/ipv4/ip_forward"))
        .and_then(|mut f| f.read_to_string())
        .map_err(|e| format!("Can't read sysctl: {}", e)));

    if nforward.as_slice().trim() == "0" {
        let mut cmd = sysctl.clone();
        cmd.arg("net.ipv4.ip_forward=1");
        commands.push(cmd);
    } else {
        info!("Sysctl is ok [{}]", nforward.as_slice().trim());
    }

    let mut cmd = sysctl.clone();
    cmd.arg("net.ipv4.conf.vagga.route_localnet=1");
    commands.push(cmd);

    let nameservers = try!(get_nameservers());
    info!("Detected nameservers: {:?}", nameservers);

    let local_dns = nameservers.as_slice() == ["127.0.0.1".to_string()];

    if !dry_run {
        try!(File::create(&netns_file)
            .map_err(|e| format!("Error creating netns file: {}", e)));
        try!(File::create(&userns_file)
            .map_err(|e| format!("Error creating userns file: {}", e)));
    }

    // If we are root we may skip sudo
    let mut mount_cmd = Command::new("sudo");
    mount_cmd.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    mount_cmd.arg("mount");

    let mut cmd = mount_cmd.clone();
    cmd.arg("--bind");
    cmd.arg(format!("/proc/{}/ns/net", child_pid));
    cmd.arg(netns_file);
    commands.push(cmd);

    let mut cmd = mount_cmd.clone();
    cmd.arg("--bind");
    cmd.arg(format!("/proc/{}/ns/user", child_pid));
    cmd.arg(userns_file);
    commands.push(cmd);

    let mut iprules = vec!();
    if local_dns {
        iprules.push(vec!("-I", "INPUT",
                          "-i", interface_name.as_slice(),
                          "-d", "127.0.0.1",
                          "-j", "ACCEPT"));
        //  The "tcp" rule doesn't actually work for now for dnsmasq
        //  because dnsmasq tries to find out real source IP.
        //  It may work for bind though.
        iprules.push(vec!("-t", "nat", "-I", "PREROUTING",
                          "-p", "tcp", "-i", "vagga",
                          "-d", host_ip.as_slice(), "--dport", "53",
                          "-j", "DNAT", "--to-destination", "127.0.0.1"));
        iprules.push(vec!("-t", "nat", "-I", "PREROUTING",
                          "-p", "udp", "-i", "vagga",
                          "-d", host_ip.as_slice(), "--dport", "53",
                          "-j", "DNAT", "--to-destination", "127.0.0.1"));
    }
    iprules.push(vec!("-t", "nat", "-A", "POSTROUTING",
                      "-s", network.as_slice(), "-j", "MASQUERADE"));


    println!("");
    println!("The following commands will be run:");
    for cmd in commands.iter() {
        println!("    {:?}", cmd);
    }



    if iptables {
        println!("");
        println!("The following iptables rules will be established:");

        for rule in iprules.iter() {
            print!("    iptables");
            for i in rule.iter() {
                print!(" {:?}", i);
            }
            println!("");
        }
    }

    if !dry_run {
        for cmd in commands.iter() {
            match cmd.status() {
                Ok(ExitStatus(0)) => {},
                val => return Err(
                    format!("Error running command {:?}: {:?}", cmd, val)),
            }
        }

        match wait_process(child_pid) {
            Ok(0) => {}
            code => return Err(
                format!("vagga_setup_netns exited with code: {:?}", code)),
        }
        if iptables {
            let mut iptables = Command::new("sudo");
            iptables.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
            iptables.arg("iptables");

            for rule in iprules.iter() {
                let mut cmd = iptables.clone();
                iptables.stderr(InheritFd(1));  // Message not an error actually
                let mut check_rule = rule.clone();
                for item in check_rule.iter_mut() {
                    if *item == "-A" || *item == "-I" {
                        *item = "-C";
                    }
                }
                cmd.args(check_rule.as_slice());
                let exists = match cmd.status() {
                    Ok(ExitStatus(0)) => true,
                    Ok(ExitStatus(1)) => false,
                    val => return Err(
                        format!("Error running command {:?}: {:?}", cmd, val)),
                };
                debug!("Checked {:?} -> {}", check_rule, exists);

                if exists {
                    info!("Rule {:?} already setup. Skipping...", rule);
                } else {
                    let mut cmd = iptables.clone();
                    cmd.args(rule.as_slice());
                    debug!("Running {:?}", rule);
                    match cmd.status() {
                        Ok(ExitStatus(0)) => {},
                        val => return Err(
                            format!("Error setting up iptables {:?}: {:?}",
                                cmd, val)),
                    }
                }
            }
        }
    }

    Ok(0)
}

pub fn destroy_netns(_config: &Config, mut args: Vec<String>)
    -> Result<i32, String>
{
    let interface_name = "vagga".to_string();
    let network = "172.18.255.0/30".to_string();
    let host_ip = "172.18.255.1".to_string();
    let mut dry_run = false;
    let mut iptables = true;
    {
        args.insert(0, "vagga _create_netns".to_string());
        let mut ap = ArgumentParser::new();
        ap.set_description("
            Set's up network namespace for subsequent container runs
            ");
        ap.refer(&mut dry_run)
            .add_option(&["--dry-run"], StoreTrue,
                "Do not run commands, only show");
        ap.refer(&mut iptables)
            .add_option(&["--no-iptables"], StoreFalse,
                "Do not remove iptables rules (useful you have firewall \
                 other than iptables). You need to update your firewall rules \
                 manually to have functional networking.");
        match ap.parse(args, &mut stdout(), &mut stderr()) {
            Ok(()) => {}
            Err(0) => return Ok(0),
            Err(_) => {
                return Ok(122);
            }
        }
    }
    let runtime_dir = namespace_dir();
    let netns_file = runtime_dir.join("netns");
    let userns_file = runtime_dir.join("userns");

    let mut commands = vec!();

    // If we are root we may skip sudo
    let mut umount_cmd = Command::new("sudo");
    umount_cmd.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    umount_cmd.arg("umount");

    let mut iptcmd = Command::new("sudo");
    iptcmd.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    iptcmd.arg("iptables");

    let mut cmd = umount_cmd.clone();
    cmd.arg(&netns_file);
    commands.push(cmd);

    let mut cmd = umount_cmd.clone();
    cmd.arg(&userns_file);
    commands.push(cmd);

    if iptables {
        let mut cmd = iptcmd.clone();
        cmd.args(&["-t", "nat", "-D", "POSTROUTING",
                   "-s", network.as_slice(), "-j", "MASQUERADE"]);
        commands.push(cmd);

        let mut cmd = iptcmd.clone();
        cmd.args(&["-D", "INPUT",
                   "-i", interface_name.as_slice(),
                   "-d", "127.0.0.1",
                   "-j", "ACCEPT"]);
        commands.push(cmd);

        let mut cmd = iptcmd.clone();
        cmd.args(&["-t", "nat", "-D", "PREROUTING",
                   "-p", "tcp", "-i", "vagga",
                   "-d", host_ip.as_slice(), "--dport", "53",
                   "-j", "DNAT", "--to-destination", "127.0.0.1"]);
        commands.push(cmd);

        let mut cmd = iptcmd.clone();
        cmd.args(&["-t", "nat", "-D", "PREROUTING",
                   "-p", "udp", "-i", "vagga",
                   "-d", host_ip.as_slice(), "--dport", "53",
                   "-j", "DNAT", "--to-destination", "127.0.0.1"]);
        commands.push(cmd);
    }

    println!("We will run network setup commands with sudo.");
    println!("You may need to enter your password.");
    println!("");
    println!("The following commands will be run:");
    for cmd in commands.iter() {
        println!("    {:?}", cmd);
    }

    if !dry_run {
        for cmd in commands.iter() {
            match cmd.status() {
                Ok(ExitStatus(0)) => {}
                val => {
                    error!("Error running command {:?}: {:?}", cmd, val);
                }
            }
        }
        if let Err(e) = remove_file(&netns_file) {
            error!("Error removing file: {}", e);
        }
        if let Err(e) = remove_file(&userns_file) {
            error!("Error removing file: {}", e);
        }
    }


    Ok(0)
}

pub fn is_netns_set_up() -> bool {
    let nsdir = namespace_dir();
    return nsdir.join("userns").exists() && nsdir.join("netns").exists();
}

pub fn join_gateway_namespaces() -> Result<(), String> {
    let nsdir = namespace_dir();
    try!(set_namespace(&nsdir.join("userns"), NewUser)
        .map_err(|e| format!("Error setting userns: {}", e)));
    try!(set_namespace(&nsdir.join("netns"), NewNet)
        .map_err(|e| format!("Error setting networkns: {}", e)));
    Ok(())
}

pub fn get_nameservers() -> Result<Vec<String>, String> {
    File::open(&Path::new("/etc/resolv.conf"))
        .map(BufReader::new)
        .and_then(|mut f| {
            let mut ns = vec!();
            for line in f.lines() {
                let line = try!(line);
                if line.as_slice().starts_with("nameserver ") {
                    ns.push(line[11..].trim().to_string());
                }
            }
            Ok(ns)
        })
        .map_err(|e| format!("Can't read resolv.conf: {}", e))
}

fn get_interfaces() -> Result<BitSet, String> {
    File::open(&Path::new("/proc/net/dev"))
        .map(BufReader::new)
        .and_then(|mut f| {
            let mut lineiter = f.lines();
            let mut result = BitSet::with_capacity(MAX_INTERFACES);
            try!(lineiter.next().unwrap());  // Two header lines
            try!(lineiter.next().unwrap());
            for line in lineiter {
                let line = try!(line);
                let line = line.trim();
                let end = line.find(':');
                if line.starts_with("ch") && end.is_some() {
                    if let Ok(num) = FromStr::from_str(
                        &line[3..end.unwrap()])
                    {
                        result.insert(num);
                    }
                }
            }
            return Ok(result);
        })
        .map_err(|e| format!("Can't read interfaces: {}", e))
}

fn get_unused_inteface_no() -> Result<usize, String> {
    // Algorithm is not perfect but should be good enough as there are 2048
    // slots in total, and average user only runs a couple of commands
    // simultaneously. It fails miserably only if there are > 100 or they
    // are spawning too often.
    let busy = try!(get_interfaces());
    let start = Range::new(0usize, MAX_INTERFACES - 100)
                .ind_sample(&mut thread_rng());
    for index in start..MAX_INTERFACES {
        if busy.contains(&index) {
            continue;
        }
        return Ok(index);
    }
    return Err(format!("Can't find unused inteface"));
}

fn _run_command(cmd: Command) -> Result<(), String> {
    debug!("Running {:?}", cmd);
    match cmd.status() {
        Ok(ExitStatus(0)) => Ok(()),
        code => Err(format!("Error running {:?}: {:?}",  cmd, code)),
    }
}

pub fn setup_bridge(link_to: &Path, port_forwards: &Vec<(u16, String, u16)>)
    -> Result<String, String>
{
    let index = try!(get_unused_inteface_no());

    let eif = format!("ch{}", index);
    let iif = format!("ch{}c", index);
    let eip = format!("172.18.{}.{}", 192 + (index*4)/256, (index*4 + 1) % 256);
    let iip = format!("172.18.{}.{}", 192 + (index*4)/256, (index*4 + 2) % 256);

    try!(File::create(link_to)
        .map_err(|e| format!("Can't create namespace file: {}", e)));

    let mut ip_cmd = Command::new("ip");
    ip_cmd.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let mut cmd = ip_cmd.clone();
    cmd.args(&["link", "add", eif.as_slice(), "type", "veth",
               "peer", "name", iif.as_slice()]);
    try!(_run_command(cmd));

    let mut cmd = ip_cmd.clone();
    cmd.args(&["addr", "add"]);
    cmd.arg(eip.clone() + "/30").arg("dev").arg(eif.as_slice());
    try!(_run_command(cmd));

    let mut cmd = ip_cmd.clone();
    cmd.args(&["link", "set", "dev", eif.as_slice(), "up"]);
    try!(_run_command(cmd));

    let cmdname = Rc::new("setup_netns".to_string());
    let mut cmd = ContainerCommand::new(cmdname.to_string(),
        env::current_exe().unwrap().parent().unwrap()
        .join("vagga_setup_netns"));
    cmd.args(&["bridge",
        "--interface", iif.as_slice(),
        "--ip", iip.as_slice(),
        "--gateway-ip", eip.as_slice(),
        "--port-forwards", &json::encode(port_forwards).unwrap()[..],
        ]);
    cmd.network_ns();
    cmd.set_env("TERM".to_string(),
                env::var("TERM").unwrap_or("dumb".to_string()));
    if let Ok(x) = env::var("PATH") {
        cmd.set_env("PATH".to_string(), x);
    }
    if let Ok(x) = env::var("RUST_LOG") {
        cmd.set_env("RUST_LOG".to_string(), x);
    }
    if let Ok(x) = env::var("RUST_BACKTRACE") {
        cmd.set_env("RUST_BACKTRACE".to_string(), x);
    }
    let pid = try!(cmd.spawn());

    let mut cmd = ip_cmd.clone();
    cmd.args(&["link", "set", "dev", iif.as_slice(),
               "netns", format!("{}", pid).as_slice()]);
    let res = bind_mount(&Path::new(format!("/proc/{}/ns/net", pid)), link_to)
        .and(_run_command(cmd));
    match wait_process(pid) {
        Ok(0) => {}
        code => return Err(
            format!("vagga_setup_netns exited with code: {:?}", code)),
    }
    match res {
        Ok(()) => Ok(iip),
        Err(e) => {
            let mut cmd = ip_cmd.clone();
            cmd.args(&["link", "del", eif.as_slice()]);
            try!(_run_command(cmd));
            Err(e)
        }
    }
}

pub fn setup_container(link_net: &Path, link_uts: &Path, name: &str,
    ip: &str, hostname: &str)
    -> Result<(), String>
{
    let eif = if name.as_bytes().len() > 14 {
        let mut hash = Sha256::new();
        hash.input(name.as_bytes());
        "eh".to_string() + hash.result_str().as_slice().slice_to(12)
    } else {
        name.to_string()
    };
    let iif = eif.clone() + "g";

    try!(File::create(link_net)
        .map_err(|e| format!("Can't create namespace file: {}", e)));
    try!(File::create(link_uts)
        .map_err(|e| format!("Can't create namespace file: {}", e)));

    let mut ip_cmd = Command::new("ip");
    ip_cmd.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let mut busybox = Command::new(
        env::current_exe().unwrap().parent().unwrap()
        .join("busybox"));
    busybox.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let mut cmd = ip_cmd.clone();
    cmd.args(&["link", "add", eif.as_slice(), "type", "veth",
               "peer", "name", iif.as_slice()]);
    try!(_run_command(cmd));

    let mut cmd = ip_cmd.clone();
    cmd.args(&["link", "set", "dev", eif.as_slice(), "up"]);
    try!(_run_command(cmd));

    let mut cmd = busybox.clone();
    cmd.args(&["brctl", "addif", "children", eif.as_slice()]);
    try!(_run_command(cmd));

    let cmdname = Rc::new("setup_netns".to_string());
    let mut cmd = ContainerCommand::new(cmdname.to_string(),
        env::current_exe().unwrap().parent().unwrap()
        .join("vagga_setup_netns"));
    cmd.args(&["guest", "--interface", iif.as_slice(),
                        "--ip", ip.as_slice(),
                        "--hostname", hostname,
                        "--gateway-ip", "172.18.0.254"]);
    cmd.network_ns();
    cmd.set_env("TERM".to_string(),
                env::var("TERM").unwrap_or("dumb".to_string()));
    if let Ok(x) = env::var("PATH") {
        cmd.set_env("PATH".to_string(), x);
    }
    if let Ok(x) = env::var("RUST_LOG") {
        cmd.set_env("RUST_LOG".to_string(), x);
    }
    if let Ok(x) = env::var("RUST_BACKTRACE") {
        cmd.set_env("RUST_BACKTRACE".to_string(), x);
    }
    let pid = try!(cmd.spawn());

    let mut cmd = ip_cmd.clone();
    cmd.args(&["link", "set", "dev", iif.as_slice(),
               "netns", format!("{}", pid).as_slice()]);
    let res = bind_mount(&Path::new(format!("/proc/{}/ns/net", pid)), link_net)
        .and(bind_mount(&Path::new(format!("/proc/{}/ns/uts", pid)), link_uts))
        .and(_run_command(cmd));
    match wait_process(pid) {
        Ok(0) => {}
        code => return Err(
            format!("vagga_setup_netns exited with code: {:?}", code)),
    }

    match res {
        Ok(()) => Ok(()),
        Err(e) => {
            let mut cmd = ip_cmd.clone();
            cmd.args(&["link", "del", eif.as_slice()]);
            try!(_run_command(cmd));
            Err(e)
        }
    }
}

impl PortForwardGuard {
    pub fn new(ns: Path, ip: String, ports: Vec<u16>) -> PortForwardGuard {
        return PortForwardGuard {
            nspath: ns,
            ip: ip,
            ports: ports,
        };
    }
    pub fn start_forwarding(&self) -> Result<(), String> {
        try!(set_namespace(&self.nspath, NewNet)
            .map_err(|e| format!("Error joining namespace: {}", e)));
        let mut iptables = Command::new("iptables");
        iptables.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());

        for port in self.ports.iter() {
            let mut cmd = iptables.clone();
            cmd.args(&["-t", "nat", "-I", "PREROUTING",
                       "-p", "tcp", "-m", "tcp",
                       "--dport", format!("{}", port).as_slice(),
                       "-j", "DNAT",
                       "--to-destination", self.ip.as_slice()]);
            try!(_run_command(cmd));
        }

        Ok(())
    }
}

impl Drop for PortForwardGuard {
    fn drop(&mut self) {
        if let Err(e) = set_namespace(&self.nspath, NewNet) {
            error!("Can't set namespace {}: {}. \
                    Unable to clean firewall rules", self.nspath.display(), e);
            return;
        }
        let mut iptables = Command::new("iptables");
        iptables.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
        for port in self.ports.iter() {
            let mut cmd = iptables.clone();
            cmd.args(&["-t", "nat", "-D", "PREROUTING",
                       "-p", "tcp", "-m", "tcp",
                       "--dport", format!("{}", port).as_slice(),
                       "-j", "DNAT",
                       "--to-destination", self.ip.as_slice()]);
            _run_command(cmd)
            .unwrap_or_else(|e| error!("Error deleting firewall rule: {}", e));
        }
    }
}
