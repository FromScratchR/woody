use std::os::unix::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <chroot_dir> <command> [args...]", args[0]);
        return;
    }

    let chroot_dir = &args[1];
    let command = &args[2];
    let command_args = &args[3..];

    // Set dummy contaner's root dir
    let root_dir = "/tmp/wood_root";
    if !std::path::Path::new(root_dir).exists() {
        std::fs::create_dir(root_dir).expect("Failed to create root dir");
    }

    let command_path = format!("{}/{}", root_dir, command);
    // Copy commands into the new root dir
    std::fs::copy(format!("/bin/{}", command), &command_path) .expect("Failed to copy command");

    //
    fs::chroot(root_dir).expect("chroot failed.");
}
