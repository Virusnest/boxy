use std::{env, fs};
use std::ffi::{CStr, CString};
use std::io::Write;
use std::path::Path;
use std::process::{exit, Command};
use caps::{CapSet, Capability};
use libc::CLONE_NEWIPC;
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::unistd::{chdir, execv, execve, fork, getgid, getuid, pivot_root, setgid, setuid, ForkResult, Gid, Uid};
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
            for cap in caps::all() {
                let _ = caps::drop(None, CapSet::Bounding, cap);
            }

            nix::unistd::write(&child_to_parent_w, &[1]).unwrap();
            println!("unshared newUser");

            let mut buf = [0u8; 1];
            nix::unistd::read(&parent_to_child_r, &mut buf).unwrap();
            println!("unshared Maps Written");
            unshare(CloneFlags::CLONE_NEWNS |CloneFlags::CLONE_NEWPID|CloneFlags::CLONE_NEWUTS|CloneFlags::CLONE_NEWIPC).unwrap();

            match unsafe { fork() } {
                Ok(ForkResult::Parent { child, .. }) => {
                    let mut exit_code = 0;
                    loop {
                        match nix::sys::wait::waitpid(None, None) {
                            Ok(nix::sys::wait::WaitStatus::Exited(pid, code)) => {
                                if pid == child {
                                    exit_code = code;
                                }
                            }
                            Ok(nix::sys::wait::WaitStatus::Signaled(pid, sig, _)) => {
                                if pid == child {
                                    exit_code = 128 + sig as i32;
                                }
                            }
                            Ok(_) => {
                                // Stopped/Continued/etc -- not a reap, keep looping.
                            }
                            Err(nix::errno::Errno::ECHILD) => break, // nothing left to reap
                            Err(nix::errno::Errno::EINTR) => continue,
                            Err(_) => break,
                        }
                    }
                    exit(exit_code);

                }
                Ok(ForkResult::Child) => {
                    mount(
                        None::<&str>,
                        "/",
                        None::<&str>,
                        MsFlags::MS_REC | MsFlags::MS_SLAVE | MsFlags::MS_SILENT,
                        None::<&str>,
                    ).unwrap();

                    // After creating the directory, bind mount it to itself
                    // // right after the tmpfs mount call, before create_dir_all
                    // println!("{}", fs::read_to_string("/proc/self/uid_map").unwrap());
                    // println!("{}", fs::read_to_string("/proc/self/gid_map").unwrap());
                    // println!("{}", fs::read_to_string("/proc/self/status").unwrap()); // check Uid:/Gid: lines
                    fs::create_dir_all(Path::new("/tmp/newroot")).expect("Failed to create newroot");
                    mount(
                        Some("tmpfs"),
                        "/tmp/newroot",
                        Some("tmpfs"),
                        MsFlags::empty(),
                        Some("mode=1777"),
                    ).unwrap();
                    mount(
                        Some("/tmp/newroot"),
                        "/tmp/newroot",
                        None::<&str>,
                        MsFlags::MS_BIND|MsFlags::MS_REC | MsFlags::MS_SILENT|MsFlags::MS_MGC_VAL,
                        None::<&str>,
                    ).unwrap();
                    fs::create_dir_all(Path::new("/tmp/newroot/oldroot")).unwrap();
                    pivot_root("/tmp/newroot", "/tmp/newroot/oldroot").unwrap();
                    chdir(Path::new("/")).unwrap();

                    create_root(Path::new("/"), home_path);

                    mount(Some("oldroot"), "oldroot", None::<&str>, MsFlags::MS_SILENT | MsFlags::MS_REC | MsFlags::MS_PRIVATE, None::<&str>).unwrap();



                    umount2("/oldroot", MntFlags::MNT_DETACH).unwrap();

                    fs::remove_dir_all("/oldroot").unwrap();
                    if (fs::exists("/usr/bin/bash").unwrap()){
                        println!("file exisits");
                    }

                    // mount(Some("sysfs"), "/sys", Some("sysfs"), MsFlags::empty(), None::<&str>).unwrap();
                    // mount(Some("devtmpfs"), "/dev", Some("devtmpfs"), MsFlags::empty(), None::<&str>).unwrap();
                    use std::os::unix::process::CommandExt;

                    // unsafe {
                    //     libc::prctl(libc::PR_SET_KEEPCAPS, 1, 0, 0, 0);
                    // }

                    // setuid(Uid::from_raw(1000)).unwrap();
                    // setgid(Gid::from_raw(1000)).unwrap();
                    setgid(Gid::from_raw(1000)).unwrap();
                    setuid(Uid::from_raw(1000)).unwrap();
                    unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 1, 0, 0, 0); }
                    // 2. Permitted set survives (thanks to keepcaps), but Effective was
                    //    cleared by the uid change. Raise CAP_SYS_PTRACE back into Effective
                    //    so crashpad's ptrace-based /proc/<pid>/mem reads succeed.
                    // caps::raise(None, CapSet::Effective, Capability::CAP_SYS_PTRACE)
                    //     .expect("raise effective CAP_SYS_PTRACE");
                    //
                    // // 3. Put it in Inheritable + Ambient so it survives the exec() into bash/Discord.
                    // caps::raise(None, CapSet::Inheritable, Capability::CAP_SYS_PTRACE)
                    //     .expect("raise inheritable CAP_SYS_PTRACE");
                    // caps::raise(None, CapSet::Ambient, Capability::CAP_SYS_PTRACE)
                    //     .expect("raise ambient CAP_SYS_PTRACE");

                    // 4. Strip every other capability so CAP_SYS_PTRACE is the only one
                    //    that survives into the sandboxed process tree.
                    // for cap in caps::all() {
                    //     if cap != Capability::CAP_SYS_PTRACE {
                    //         let _ = caps::drop(None, CapSet::Permitted, cap);
                    //         let _ = caps::drop(None, CapSet::Inheritable, cap);
                    //     }
                    // }
                    //
                    // unsafe {
                    // //     libc::prctl(libc::PR_SET_DUMPABLE, 1, 0, 0, 0);
                    // //     libc::prctl(libc::PR_SET_PTRACER, -1isize as libc::c_ulong, 0, 0, 0);
                    // //     // right before Command::new(...).exec()
                    // //     // install_seccomp_filter().expect("failed to install seccomp filter");
                    // //
                    // //     // Clear ambient caps so bwrap doesn't trip the "Unexpected capabilities" error
                    // //     // caps::clear(None, CapSet::Ambient).unwrap();
                    // //     // caps::clear(None, CapSet::Inheritable).unwrap();
                    // //     // caps::clear(None, CapSet::Permitted).unwrap();
                    // //     // caps::clear(None, CapSet::Effective).unwrap();
                    // //
                    // //     // Now safely exec Steam
                    // }
                    match unsafe { fork() }.unwrap() {
                        ForkResult::Child => {
                            unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0); }

                            let err = Command::new("/usr/bin/bash")
                                .exec(); // replaces this process's image; only returns on failure
                            eprintln!("failed to exec shell: {err}");
                            exit(1);
                        }
                        ForkResult::Parent { child: shell_pid, .. } => {
                            let mut exit_code = 0;
                            loop {
                                match nix::sys::wait::waitpid(None, None) {
                                    Ok(nix::sys::wait::WaitStatus::Exited(pid, code)) => {
                                        if pid == shell_pid {
                                            exit_code = code;
                                        }
                                    }
                                    Ok(nix::sys::wait::WaitStatus::Signaled(pid, sig, _)) => {
                                        eprintln!("PID {} died from signal {:?}", pid, sig);
                                        if pid == shell_pid {
                                            exit_code = 128 + sig as i32;
                                        }
                                    }
                                    Ok(_) => {}
                                    Err(nix::errno::Errno::ECHILD) => break, // nothing left to reap
                                    Err(nix::errno::Errno::EINTR) => continue,
                                    Err(_) => break,
                                }
                            }
                            exit(exit_code);
                        }
                    }


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

use libseccomp::{ScmpFilterContext, ScmpAction, ScmpArch, ScmpFilterAttr};

fn install_seccomp_filter() -> Result<(), libseccomp::error::SeccompError> {
    // 1. Initialize your filter context as Allow-by-default (or your chosen base action)
    let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;

    // 2. FIX: Unset the BADARCH attribute constraint if it's implicitly blocking you
    // // This tells libseccomp to gracefully accept architecture transitions instead of tossing EACCES
    // ctx.set_filter_attr(ScmpFilterAttr::ActBadArch, 0)?;
    //
    // // 3. Now register the 32-bit x86 architecture safely
    // ctx.add_arch(ScmpArch::X86)?;
    //
    // // 4. Your existing unshare/clone modifications go here...
    // // (Ensure clone3 is completely allowed for both architectures)
    // if let Ok(clone3) = libseccomp::ScmpSyscall::from_name("clone3") {
    //     ctx.add_rule(ScmpAction::Allow, clone3)?;
    // }

    // 5. Load the composite dual-architecture BPF table into the kernel
    ctx.load()?;
    Ok(())
}