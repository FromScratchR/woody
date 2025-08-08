use nix::{sched::{unshare, CloneFlags}};
use nix::unistd::{fork, ForkResult};
use std::io::{Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    daemonize()?;

    loop {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/rust_daemon.log")?;

        let timestamp = std::time::SystemTime::now();
        writeln!(file, "[{:?}] Daemon is running, PID: {}", timestamp, std::process::id())?;

        std::thread::sleep(std::time::Duration::from_secs(5))
    }
}

fn daemonize() -> Result<(), Box<dyn std::error::Error>> {
    match unsafe { libc::fork() } {
        -1 => return Err("Failed to fork".into()),
        /* child process exec point */
        0 => {
            if unsafe { libc::setsid() } == -1 {
                return Err("Failed to create new session".into())
            }

            std::env::set_current_dir("/")?;

            unsafe {

                libc::close(0); // stdin
                libc::close(1); // stdout
                libc::close(2); // stderr
            }

            Ok(())
        },
        _ => {
            /* Kill main process which acts just like a replicator */
            println!("closed by exit!!");
            std::process::exit(0);
        }
    }
}

fn init_handler(args: &[String]) {
    // Get 2 fork results by waiting child and callbacking handler fn on child process itself
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            println!("[Main] Container process created with PID: {}", child);

            match nix::sys::wait::waitpid(child, None) {
                Ok(status) => println!("[Parent]: Container precess exited with status: {:?}", status),
                Err(e) => eprintln!("[Parent] waitpid failed: {}", e)
            }
        }
        Ok(ForkResult::Child) => {
            setup_manager(&args[1..])
        }
        Err(e) => eprintln!("Could not fork: {}", e)
    }
}

fn setup_manager(_args: &[String]) {
    println!("[Container Manager] Setting up container for PID: {}", std::process::id());

    let flags = CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWUTS;
    unshare(flags).expect("Could not unshare namespaces");

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            nix::sys::wait::waitpid(child, None).expect("waitpid on grandchild failed.");
        }
        Ok(ForkResult::Child) => {
            println!("[Child {}] Success init.", std::process::id())
        }
        Err(e) => eprintln!("[Container Manager] Fork failed: {}", e) }
}
