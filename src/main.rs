use nix::{sched::{unshare, CloneFlags}};
use nix::unistd::{fork, ForkResult};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <command> [args...]", &args[0]);

        return;
    }

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
            nix::sys::wait::waitpid(child, None).expect("waitpid on grandchild failed");
        }
        Ok(ForkResult::Child) => {
            println!("SUCCESS! Child created")
        }
        Err(e) => eprintln!("[Container Manager] Fork failed {}", e)
    }
}
