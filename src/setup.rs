use std::fs;
use std::fs::File;
use nix::mount::{mount, MsFlags};
use std::path::Path;
use crate::sandbox::FileSystemType;
fn create_home(path: &Path, path_type: FileSystemType){

}


pub fn create_root(path: &Path, home_path: &Path){
    mount(
        None::<&str>,
        "/",
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None::<&str>,
    ).unwrap();

    let usr = path.join("usr");
    let dev = path.join("dev");
    let etc = path.join("etc");
    let home = path.join("home");
    let proc = path.join("proc");
    let run = path.join("run");
    let sys = path.join("sys");
    let tmp = path.join("tmp");
    let var = path.join("var");

    let wayland = path.join("run/user/1000/");

    fs::create_dir_all(&usr).unwrap();

    fs::create_dir_all(&dev).unwrap();
    fs::create_dir_all(&etc).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&proc).unwrap();
    fs::create_dir_all(&run).unwrap();
    fs::create_dir_all(&sys).unwrap();
    fs::create_dir_all(&tmp).unwrap();
    fs::create_dir_all(&var).unwrap();
    fs::create_dir_all(&var.join("tmp")).unwrap();

    fs::create_dir_all(&wayland).unwrap();
    File::create(&wayland.join("wayland-1")).unwrap();
    File::create(&wayland.join("bus")).unwrap();

    fs::write(
        &etc.join("passwd"),
        "\
root:x:0:0:root:/root:/bin/sh\n\
user:x:1000:1000:user:/home/user:/bin/sh\n"
    ).unwrap();

    bind_mount(Path::new("/usr") ,&usr,true).unwrap();
    bind_mount(Path::new("/run/user/1000/wayland-1") ,&wayland.join("wayland-1"),false).unwrap();
    bind_mount(Path::new("/run/user/1000/bus") ,&wayland.join("bus"),false).unwrap();
    fs::create_dir_all(&run.join("host/monitor")).unwrap();
    File::create(&run.join("host/monitor/resolv.conf")).unwrap();
    File::create(&etc.join("resolv.conf")).unwrap();

    bind_mount(Path::new("/etc/resolv.conf"), &run.join("host/monitor/resolv.conf"),true).unwrap();

    bind_mount(Path::new("/etc/resolv.conf"), &etc.join("resolv.conf"),true).unwrap();
    bind_mount(&home_path, &home,false).unwrap();
    create_dev(&path);
    // create_sys(&path);
    // mount(Some("devtmpfs"), &dev, Some("devtmpfs"), MsFlags::empty(), None::<&str>).unwrap();

    std::os::unix::fs::symlink(Path::new("/usr/bin"),path.join("bin") ).unwrap();
    std::os::unix::fs::symlink(Path::new("/usr/lib"),path.join("lib") ).unwrap();
    std::os::unix::fs::symlink(Path::new("/usr/lib"),path.join("lib64") ).unwrap();

    mount(
        Some("tmpfs"),
        &tmp,
        Some("tmpfs"),
        MsFlags::empty(),
        Some("mode=1777"),
    ).unwrap();

    mount(
        Some("tmpfs"),
        &var.join("tmp"),
        Some("tmpfs"),
        MsFlags::empty(),
        Some("mode=1777"),
    ).unwrap();

}

fn create_sys(path:&Path){
    fs::create_dir_all(&path.join("sys/block")).unwrap();
    fs::create_dir_all(&path.join("sys/bus")).unwrap();
    fs::create_dir_all(&path.join("sys/class")).unwrap();
    fs::create_dir_all(&path.join("sys/dev")).unwrap();
    fs::create_dir_all(&path.join("sys/devices")).unwrap();
    bind_mount(Path::new("/sys/block"), &path.join("sys/block"),true).unwrap();
    bind_mount(Path::new("/sys/bus"), &path.join("sys/bus"),true).unwrap();
    bind_mount(Path::new("/sys/class"), &path.join("sys/class"),true).unwrap();
    bind_mount(Path::new("/sys/dev"), &path.join("sys/dev"),true).unwrap();
    bind_mount(Path::new("/sys/devices"), &path.join("sys/devices"),true).unwrap();
}
fn create_dev(path: &Path) {
    let dev = path.join("dev");
    fs::create_dir_all(&dev).unwrap();
    mount(Some("tmpfs"), &dev, Some("tmpfs"), MsFlags::empty(), Some("mode=755")).unwrap();

    for name in ["null", "zero", "full", "random", "urandom", "tty"] {
        let target = dev.join(name);
        File::create(&target).unwrap();
        bind_mount(Path::new(&format!("/dev/{}", name)), &target, false).unwrap();
    }

    let shm = dev.join("shm");
    fs::create_dir_all(&shm).unwrap();
    mount(Some("tmpfs"), &shm, Some("tmpfs"), MsFlags::empty(), Some("mode=1777")).unwrap();

    let pts = dev.join("pts");
    fs::create_dir_all(&pts).unwrap();
    mount(Some("devpts"), &pts, Some("devpts"), MsFlags::empty(), Some("newinstance,ptmxmode=0666,mode=620")).unwrap();

    fs::create_dir_all(&dev.join("dri")).unwrap();
    bind_mount(Path::new("/dev/dri"),&dev.join("dri"), false).unwrap();

    std::os::unix::fs::symlink("pts/ptmx", dev.join("ptmx")).unwrap();

    std::os::unix::fs::symlink("/proc/self/fd", dev.join("fd")).unwrap();
    std::os::unix::fs::symlink("/proc/self/fd/0", dev.join("stdin")).unwrap();
    std::os::unix::fs::symlink("/proc/self/fd/1", dev.join("stdout")).unwrap();
    std::os::unix::fs::symlink("/proc/self/fd/2", dev.join("stderr")).unwrap();
}

fn bind_mount(from: &Path, to: &Path, ro: bool) -> nix::Result<()> {
    let mut result = mount(
        Some(from),
        to,
        None::<&str>,
        MsFlags::MS_BIND|MsFlags::MS_REC,
        None::<&str>,
    );
    if ro {
        result = mount(
            None::<&str>,
            to,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
            None::<&str>,
        );
    }
    result
}