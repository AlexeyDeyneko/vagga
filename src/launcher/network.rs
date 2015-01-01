use std::rc::Rc;
use std::io::{stdout, stderr};
use std::os::{getenv, self_exe_path};
use std::io::{USER_RWX};
use std::io::fs::{File, PathExtensions};
use std::io::fs::{mkdir};
use std::io::process::{Command, Ignored, InheritFd, ExitStatus};
use libc::funcs::posix88::unistd::{geteuid};

use argparse::{ArgumentParser, StoreTrue, StoreFalse};

use config::Config;
use container::util::get_user_name;
use container::monitor::{Monitor, Exit, Killed, RunOnce};


pub fn create_netns(_config: &Config, mut args: Vec<String>)
    -> Result<int, String>
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
            .add_option(&["--dry-run"], box StoreTrue,
                "Do not run commands, only show");
        ap.refer(&mut iptables)
            .add_option(&["--no-iptables"], box StoreFalse,
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

    let uid = unsafe { geteuid() };
    let runtime_dir = getenv("XDG_RUNTIME_DIR")
        .map(|v| Path::new(v).join("vagga"))
        .unwrap_or(Path::new(format!("/tmp/vagga-{}", get_user_name(uid))));
    if !runtime_dir.exists() {
        try!(mkdir(&runtime_dir, USER_RWX)
            .map_err(|e| format!("Can't create runtime_dir: {}", e)));
    }

    let netns_file = runtime_dir.join("netns");
    let userns_file = runtime_dir.join("userns");

    if netns_file.exists() || userns_file.exists() {
        return Err("Namespaces already created".to_string());
    }

    let mut mon = Monitor::new();
    let vsn = Rc::new("vagga_setup_netns".to_string());
    {
        use container::container::Command;
        let mut cmd = Command::new("setup_netns".to_string(),
            self_exe_path().unwrap().join("vagga_setup_netns"));
        cmd.set_max_uidmap();
        cmd.network_ns();
        cmd.arg("--guest-ip");
        cmd.arg(guest_ip.as_slice());
        cmd.arg("--gateway-ip");
        cmd.arg(host_ip.as_slice());
        cmd.arg("--network");
        cmd.arg(network.as_slice());
        mon.add(vsn.clone(), box RunOnce::new(cmd));
    }
    let child_pid = if dry_run { 123456 } else { try!(mon.force_start(vsn)) };

    println!("We will run network setup commands with sudo.");
    println!("You may need to enter your password.");

    let mut commands = vec!();

    // If we are root we may skip sudo
    let mut ip_cmd = Command::new("sudo");
    ip_cmd.stdin(Ignored).stdout(InheritFd(1)).stderr(InheritFd(2));
    ip_cmd.arg("ip");

    let mut cmd = ip_cmd.clone();
    cmd.args(["link", "add", "vagga_guest", "type", "veth",
              "peer", "name", interface_name.as_slice()]);
    commands.push(cmd);

    let mut cmd = ip_cmd.clone();
    cmd.args(["link", "set", "vagga_guest", "netns"]);
    cmd.arg(format!("{}", child_pid));
    commands.push(cmd);

    let mut cmd = ip_cmd.clone();
    cmd.args(["addr", "add", host_ip_net.as_slice(),
              "dev", interface_name.as_slice()]);
    commands.push(cmd);

    let nforward = try!(File::open(&Path::new("/proc/sys/net/ipv4/ip_forward"))
        .and_then(|mut f| f.read_to_string())
        .map_err(|e| format!("Can't read sysctl: {}", e)));

    if nforward.as_slice().trim() == "0" {
        // If we are root we may skip sudo
        let mut cmd = Command::new("sudo");
        cmd.stdin(Ignored).stdout(InheritFd(1)).stderr(InheritFd(2));
        cmd.args(["sysctl", "net.ipv4.ip_forward=1"]);
        commands.push(cmd);
    } else {
        info!("Sysctl is ok [{}]", nforward.as_slice().trim());
    }

    if !dry_run {
        try!(File::create(&netns_file)
            .map_err(|e| format!("Error creating netns file: {}", e)));
        try!(File::create(&userns_file)
            .map_err(|e| format!("Error creating userns file: {}", e)));
    }

    // If we are root we may skip sudo
    let mut mount_cmd = Command::new("sudo");
    mount_cmd.stdin(Ignored).stdout(InheritFd(1)).stderr(InheritFd(2));
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

    println!("");
    println!("The following commands will be run:");
    for cmd in commands.iter() {
        println!("    {}", cmd);
    }

    if !dry_run {
        for cmd in commands.iter() {
            match cmd.status() {
                Ok(ExitStatus(0)) => {},
                val => return Err(
                    format!("Error running command {}: {}", cmd, val)),
            }
        }
    }

    if iptables {
        println!("");
        println!("Checking firewall rules:");
        let mut iptables = Command::new("sudo");
        iptables.stdin(Ignored).stdout(InheritFd(1)).stderr(InheritFd(2));
        iptables.arg("iptables");

        let mut cmd = iptables.clone();
        iptables.stderr(InheritFd(1));  // Message not an error actually
        cmd.args(["-t", "nat", "-C", "POSTROUTING",
                  "-s", network.as_slice(), "-j", "MASQUERADE"]);
        println!("    {}", cmd);
        let exists = match cmd.status() {
            Ok(ExitStatus(0)) => true,
            Ok(ExitStatus(1)) => false,
            val => return Err(
                format!("Error running command {}: {}", cmd, val)),
        };

        if exists {
            println!("Already setup. Skipping...");
        } else {
            let mut cmd = iptables.clone();
            cmd.args(["-t", "nat", "-A", "POSTROUTING",
                      "-s", network.as_slice(), "-j", "MASQUERADE"]);
            println!("Not existent, creating:");
            println!("    {}", cmd);
            if !dry_run {
                match cmd.status() {
                    Ok(ExitStatus(0)) => {},
                    val => return Err(
                        format!("Error setting up iptables {}: {}",
                            cmd, val)),
                }
            }
        }
    }
    if !dry_run {
        match mon.run() {
            Exit(0) => {}
            Killed => return Err(format!("vagga_setup_netns is dead")),
            Exit(c) => return Err(
                format!("vagga_setup_netns exited with code: {}", c)),
        }
    }

    Ok(0)
}
