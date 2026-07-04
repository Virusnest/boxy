use std::{env, fs};

use std::io::Write;
use std::path::Path;
use std::process::{exit, Command};
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::unistd::{chdir, execve, fork, getgid, getuid, pivot_root, setgid, setuid, ForkResult, Gid, Uid};
use nix::sched::{unshare, CloneFlags};
use crate::setup::create_root;

mod sandbox;
mod setup;

fn main() {
    println!("Hello, world!");
    let args: Vec<String> = env::args().collect();
    match unsafe { fork() } {
        Ok(ForkResult::Child) => {

            setup_sandbox(Path::new(args[1].as_str()))

        }
        Ok(ForkResult::Parent { child, .. }) => {
            match nix::sys::wait::waitpid(child, None) {
                Ok(nix::sys::wait::WaitStatus::Exited(_, code)) => exit(code),
                _ => exit(0),
            }
        }
        Err(_) => {}
    }
}

fn setup_sandbox(home_path: &Path) {

    let (parent_to_child_r, parent_to_child_w) = nix::unistd::pipe().expect("Failed to create synchronization pipe");
    let (child_to_parent_r, child_to_parent_w) = nix::unistd::pipe().expect("Failed to create synchronization pipe");

    let host_uid = getuid();
    let host_gid = getgid();
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            println!("waiting for unshare");
            let mut buf = [0u8; 1];
            nix::unistd::read(&child_to_parent_r, &mut buf).unwrap();
            println!("unshared");

            // IMPORTANT: disable setgroups first
            let setgroups = format!("/proc/{}/setgroups", child);
            std::fs::write(setgroups, "deny").unwrap();

            // map UID 0 -> host UID 1000
            let uid_map = format!("/proc/{}/uid_map", child);
            std::fs::write(uid_map, format!("1000 {} 1\n",host_uid)).unwrap();

            // map GID 0 -> host GID 1000
            let gid_map = format!("/proc/{}/gid_map", child);
            std::fs::write(gid_map, format!("1000 {} 1\n",host_gid)).unwrap();

            println!("uid_maps written");
            nix::unistd::write(&parent_to_child_w, &[1]).unwrap();

            match nix::sys::wait::waitpid(child, None) {
                Ok(nix::sys::wait::WaitStatus::Exited(_, code)) => exit(code),
                _ => exit(0),
            }
        }
        Ok(ForkResult::Child) => {

            unshare(CloneFlags::CLONE_NEWUSER).unwrap();


            nix::unistd::write(&child_to_parent_w, &[1]).unwrap();
            println!("unshared newUser");

            let mut buf = [0u8; 1];
            nix::unistd::read(&parent_to_child_r, &mut buf).unwrap();
            println!("unshared Maps Written");
            unshare(CloneFlags::CLONE_NEWNS |CloneFlags::CLONE_NEWPID).unwrap();

            match unsafe { fork() } {
                Ok(ForkResult::Parent { child, .. }) => {
                    match nix::sys::wait::waitpid(child, None) {
                        Ok(nix::sys::wait::WaitStatus::Exited(_, code)) => exit(code),
                        _ => exit(0),
                    }
                }
                Ok(ForkResult::Child) => {
                    mount(
                        None::<&str>,
                        "/",
                        None::<&str>,
                        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
                        None::<&str>,
                    ).unwrap();
                    mount(
                        Some("tmpfs"),
                        "/tmp",
                        Some("tmpfs"),
                        MsFlags::empty(),
                        Some("mode=1777"),
                    ).unwrap();
                    // After creating the directory, bind mount it to itself
                    // // right after the tmpfs mount call, before create_dir_all
                    // println!("{}", fs::read_to_string("/proc/self/uid_map").unwrap());
                    // println!("{}", fs::read_to_string("/proc/self/gid_map").unwrap());
                    // println!("{}", fs::read_to_string("/proc/self/status").unwrap()); // check Uid:/Gid: lines
                    fs::create_dir_all(Path::new("/tmp/newroot")).expect("Failed to create newroot");

                    mount(
                        Some("/tmp/newroot"),
                        "/tmp/newroot",
                        None::<&str>,
                        MsFlags::MS_BIND,
                        None::<&str>,
                    ).unwrap();
                    setuid(Uid::from_raw(1000)).unwrap();

                    create_root(Path::new("/tmp/newroot"), home_path);


                    fs::create_dir_all(Path::new("/tmp/newroot/tmp/oldroot")).unwrap();

                    println!("{}", std::fs::read_to_string("/proc/self/status").unwrap());
                    pivot_root("/tmp/newroot", "/tmp/newroot/tmp/oldroot").unwrap();
                    chdir(Path::new("/")).unwrap();

                    mount(Some("proc"), "/proc", Some("proc"), MsFlags::empty(), None::<&str>).unwrap();

                    umount2("/tmp/oldroot", MntFlags::MNT_DETACH).unwrap();

                    fs::remove_dir_all("/tmp/oldroot").unwrap();
                    if (fs::exists("/usr/bin/bash").unwrap()){
                        println!("file exisits");
                    }
                    // setgid(Gid::from_raw(1000)).unwrap();

                    // mount(Some("sysfs"), "/sys", Some("sysfs"), MsFlags::empty(), None::<&str>).unwrap();
                    // mount(Some("devtmpfs"), "/dev", Some("devtmpfs"), MsFlags::empty(), None::<&str>).unwrap();
                    Command::new("/usr/bin/bash").arg("-c").arg("/usr/bin/bash").status().unwrap();

                }
                Err(_) => {}
            }

            // match unsafe { fork() } {
            //     Ok(ForkResult::Parent { child, .. }) => {
            //         match nix::sys::wait::waitpid(child, None) {
            //             Ok(status) => {
            //                 // Forward the shell's exit status out to the host parent
            //                 match status {
            //                     nix::sys::wait::WaitStatus::Exited(_, code) => exit(code),
            //                     _ => exit(0),
            //                 }
            //             }
            //             Err(_) => exit(1),
            //         }
            //     }
            //     Ok(ForkResult::Child) => {
            //
            //
            //         println!("Launching genuinely isolated shell...");
            //         for entry in fs::read_dir(Path::new("/usr/bin")).unwrap() {
            //             let entry = entry.unwrap();
            //             let name = entry.file_name();
            //             println!("{:?}", name);
            //         }
            //         let path = CString::new("/usr/bin/bash").unwrap();
            //         let args = [
            //             CString::new("/usr/bin/bash").unwrap(),
            //         ];
            //         // Explicitly set the HOME env variable to our internal isolated target path
            //         let env = [
            //             CString::new("HOME=/").unwrap(),
            //             CString::new("XDG_RUNTIME_DIR=/run/user/1000").unwrap()
            //         ];
            //
            //         match execve(&path, &args, &env) {
            //             Ok(_) => {}
            //             Err(err) => {
            //                 eprintln!("Failed to execute shell: {}", err);
            //                 exit(1);
            //             }
            //         }
            //     }
            //     Err(_) => {}
            // }
        }
        Err(_) => println!("Fork failed"),
    }


}