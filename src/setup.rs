use std::fs;
use std::fs::{File, Permissions};
use std::os::unix::fs::PermissionsExt;
use nix::mount::{mount, MsFlags};
use std::path::Path;
use libc::bind;
use crate::sandbox::FileSystemType;
use crate::sandbox::FileSystemType::GoCryptFS;

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
    let proc = path.join("/proc");
    let opt = path.join("opt");
    let run = path.join("run");
    let srv = path.join("srv");

    let sys = path.join("sys");
    let tmp = path.join("tmp");
    let var = path.join("var");

    let user = path.join("run/user/1000/");


    fs::create_dir_all(&usr).unwrap();

    fs::create_dir_all(&dev).unwrap();
    fs::set_permissions(&dev, Permissions::from_mode(0o755)).unwrap();

    fs::create_dir_all(&etc).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&proc).unwrap();
    fs::set_permissions(&proc, Permissions::from_mode(0o755)).unwrap();

    mount(Some("proc"), &proc, Some("proc"), MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV, None::<&str>).unwrap();

    fs::create_dir_all(&run).unwrap();
    // fs::create_dir_all(&sys).unwrap();
    fs::create_dir_all(&opt).unwrap();
    fs::create_dir_all(&tmp).unwrap();
    fs::create_dir_all(&var).unwrap();
    fs::write(
        &etc.join("passwd"),
        "\
root:x:0:0:root:/root:/bin/sh\n\
user:x:1000:1000:user:/home/user:/bin/sh\n"
    ).unwrap();
    create_dev(&path);

    bind_mount(Path::new("/oldroot/srv"),&srv,true).unwrap();
    bind_mount(Path::new("/oldroot/opt") ,&opt,true).unwrap();

    bind_mount(Path::new("/oldroot/usr") ,&usr,true).unwrap();
    bind_mount(Path::new("/oldroot/tmp/.X11-unix") ,&tmp.join(".X11-unix"),true).unwrap();

    bind_mount(Path::new("/oldroot/run/user/1000/wayland-1"), &user.join("wayland-1"), false).unwrap();
    bind_mount(Path::new("/oldroot/run/user/1000/pulse"), &user.join("pulse"), false).unwrap();
    bind_mount(Path::new("/oldroot/run/user/1000/speech-dispatcher"), &user.join("speech-dispatcher"), false).unwrap();
    bind_mount(Path::new("/oldroot/run/user/1000/keyring"), &user.join("keyring"), false).unwrap();
    bind_mount(Path::new("/oldroot/run/user/1000/pipewire-0"), &user.join("pipewire-0"), false).unwrap();
    bind_mount(Path::new("/oldroot/run/user/1000/bus"), &user.join("bus"), false).unwrap();
    bind_mount(Path::new("/oldroot/run/dbus/system_bus_socket"), &run.join("dbus/system_bus_socket"), false).unwrap();
    bind_mount(Path::new("/oldroot/run/user/1000/doc"), &user.join("doc"), false).unwrap();


    bind_mount(
        Path::new("/oldroot/etc/machine-id"),
        &etc.join("machine-id"),
        true,
    ).unwrap();

    //
    bind_mount(Path::new("/oldroot/etc/resolv.conf"), &run.join("host/monitor/resolv.conf"),true).unwrap();
    bind_mount(Path::new("/oldroot/etc/resolv.conf"), &etc.join("resolv.conf"),true).unwrap();
    bind_mount(Path::new("/oldroot/etc/hosts"), &etc.join("hosts"),true).unwrap();

    bind_mount(Path::new("/oldroot/etc/pulse"), &etc.join("pulse"),true).unwrap();
    bind_mount(Path::new("/oldroot/etc/ca-certificates"), &etc.join("ca-certificates"),false).unwrap();
    bind_mount(Path::new("/oldroot/etc/ssl"), &etc.join("ssl"),false).unwrap();

    bind_mount(Path::new("/oldroot/etc/host.conf"), &etc.join("host.conf"),true).unwrap();
    bind_mount(Path::new("/oldroot/etc/nsswitch.conf"), &etc.join("nsswitch.conf"),true).unwrap();

    bind_mount(Path::new("/oldroot/etc/fonts"), &etc.join("fonts"),true).unwrap();

    println!("{:?}", (Path::new("/oldroot").join(&home_path.strip_prefix("/").unwrap())));
    bind_mount(&*Path::new("/oldroot").join(&home_path.strip_prefix("/").unwrap()), &home, false).unwrap();
    create_sys(&path);
    // mount(Some("devtmpfs"), &dev, Some("devtmpfs"), MsFlags::empty(), None::<&str>).unwrap();

    std::os::unix::fs::symlink(Path::new("/usr/bin"),path.join("bin") ).unwrap();
    std::os::unix::fs::symlink(Path::new("/usr/bin"),path.join("sbin") ).unwrap();

    std::os::unix::fs::symlink(Path::new("/usr/lib"),path.join("lib") ).unwrap();
    std::os::unix::fs::symlink(Path::new("/usr/lib"),path.join("lib64") ).unwrap();

    mount(
        Some("tmpfs"),
        &tmp,
        Some("tmpfs"),
        MsFlags::empty(),
        Some("mode=0755"),
    ).unwrap();

    fs::create_dir_all(&var.join("tmp")).unwrap();
    mount(
        Some("tmpfs"),
        &var.join("tmp"),
        Some("tmpfs"),
        MsFlags::empty(),
        Some("mode=0755"),
    ).unwrap();

    bind_mount(Path::new("/oldroot/tmp/.X11-unix"),&tmp.join(".X11-unix"),false).unwrap();

}

fn create_sys(path: &Path) {
    for sub in ["sys/block", "sys/bus", "sys/class", "sys/dev", "sys/devices"] {
        let src = Path::new("/oldroot/").join(sub);
        if !src.exists() {
            // Not every kernel/host exposes all five (e.g. containers-in-containers).
            continue;

        }
        let dest = path.join(sub);
        fs::create_dir_all(&dest).unwrap();
        bind_mount(&src, &dest, false).unwrap();
    }
}

fn create_dev(path: &Path) {
    let dev = path.join("/dev");
    fs::create_dir_all(&dev).unwrap();
    mount(Some("tmpfs"), &dev, Some("tmpfs"), MsFlags::empty(), Some("mode=755")).unwrap();

    for name in ["null", "zero", "full", "random", "urandom", "tty"] {
        let target = dev.join(name);
        File::create(&target).unwrap();
        fs::set_permissions(&target, Permissions::from_mode(0o444)).unwrap();
        bind_mount(Path::new(&format!("/oldroot/dev/{}", name)), &target, false).unwrap();
    }

    std::os::unix::fs::symlink("/proc/self/fd/0", "/dev/stdin").unwrap();
    std::os::unix::fs::symlink("/proc/self/fd/1", "/dev/stdout").unwrap();
    std::os::unix::fs::symlink("/proc/self/fd/2", "/dev/stderr").unwrap();

    std::os::unix::fs::symlink("/proc/self/fd", "/dev/fd").unwrap();
    std::os::unix::fs::symlink("/proc/kcore","/dev/core").unwrap();


    let shm = dev.join("shm");
    let pts = dev.join("pts");

    fs::create_dir_all(&shm).unwrap();
    fs::set_permissions(&shm, Permissions::from_mode(0o755)).unwrap();
    fs::create_dir_all(&pts).unwrap();
    fs::set_permissions(&pts, Permissions::from_mode(0o755)).unwrap();

    mount(Some("devpts"), &pts, Some("devpts"), MsFlags::MS_NOSUID|MsFlags::MS_NOEXEC,
          Some("newinstance,ptmxmode=0666,mode=620")).unwrap();

    fs::create_dir_all(&dev.join("dri")).unwrap();
    bind_mount(Path::new("/oldroot/dev/dri"), &dev.join("dri"), false).unwrap();

    // fs::create_dir_all(&dev.join("snd")).unwrap();
    // bind_mount(Path::new("/oldroot/dev/snd"), &dev.join("snd"), false).unwrap();


    std::os::unix::fs::symlink("/dev/pts/ptmx", "/dev/ptmx").unwrap();


}

fn bind_mount(from: &Path, to: &Path, ro: bool) -> nix::Result<()> {
    if(!to.exists()){
        if(!from.is_dir()){
            println!("creating File at {}", to.display());
            fs::create_dir_all(to.parent().unwrap()).unwrap();
            File::create(&to).unwrap();
        }else {
            println!("creating path at {}", to.display());
            fs::create_dir_all(&to).unwrap();
        }
    }
    let mut result = mount(
        Some(from),
        to,
        None::<&str>,
        MsFlags::MS_BIND|MsFlags::MS_REC|MsFlags::MS_NOSUID|MsFlags::MS_NODEV|(if ro {MsFlags::MS_RDONLY}else{MsFlags::empty()}),
        None::<&str>,
    );
    result
}