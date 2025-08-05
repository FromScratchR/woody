use std::{path::Path, process::{Command, Stdio}};
use nix::{sched::{clone, unshare, CloneFlags}, sys::wait::WaitPidFlag, unistd::chdir};
use nix::unistd::{chroot, fork, ForkResult};

fn main() {
    // let args: Vec<String> = std::env::args().collect();
    // if args.len() < 2 {
    //     eprintln!("Usage: {} <command> [args...]", &args[0]);
    //     return;
    // }

    // let flags = CloneFlags::CLONE_NEWUTS | CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNET;
    let flags = CloneFlags::CLONE_NEWPID;
    let mut child_stack = [0; 4096];

    let child_fn = || -> isize {
        println!("On child_process!! PID: {}", std::process::id());
        std::thread::sleep(std::time::Duration::from_secs(3));
        0
    };

    match clone(Box::new(child_fn), &mut child_stack, flags, None) {
        Ok(pid) => {
            println!("From Parent: Child created! PID: {}", pid);
                match nix::sys::wait::waitpid(pid, None) {
                    Ok(status) => println!("Child exited with status: {:?}", status),
                    Err(e) => eprintln!("Waitipid failed: {}", e)
                }
        }
        Err(e) => eprintln!("clone failed: {}, ", e)
    };

    // let pid = clone(Box::new(child_fn), &mut child_stack, flags, None).expect("Could not start new process");
    // dbg!(&pid);
    // nix::sys::wait::waitpid(pid, None).expect("failed waitpid");
}

fn children(args: &[String]) {
    println!("On child process!! PID: {}", nix::unistd::getpid());

    nix::unistd::sethostname("woody_container").expect("Could not define hostname namespace");

    let root_dir = "/tmp/woody_root";
    if !std::path::Path::new(root_dir).exists() {
        std::fs::create_dir_all(root_dir).expect("Could not create root dir.")
    }

    let command = &args[0];
    let command_path = format!("{}/bin", root_dir);

    std::fs::create_dir_all(&command_path).expect("Could not create command dir");
    std::fs::copy(format!("/bin/{}", command), format!("{}/{}", command_path, command))
        .expect("Could copy command");

    chroot(root_dir).expect("Could not chroot *root_dir*");
    chdir("/").expect("Could not change cwd");

    nix::mount::mount::<_, _, _, _>(
        Some("proc"),
        "/proc",
        Some("proc"),
        nix::mount::MsFlags::empty(),
        Some(""),
    ).expect("Could not mount /proc");

    let mut children = Command::new(format!("/bin/{}", command))
        .args(&args[1..])
        .spawn()
        .expect("Could not find create child proccess");

    children.wait().expect("Could not exec command");
}
