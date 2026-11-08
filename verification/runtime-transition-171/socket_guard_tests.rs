use super::*;
use windows_sys::Win32::Networking::WinSock::{bind, closesocket, socket, WSACleanup, WSAGetLastError, WSAStartup, SOCKADDR, WSADATA};

struct OwnSocket(usize);
impl Drop for OwnSocket {
    fn drop(&mut self) { unsafe { closesocket(self.0); WSACleanup(); } }
}

fn own_socket(path: &Path) -> OwnSocket {
    #[repr(C)]
    struct Address { family: u16, path: [u8; 108] }
    let name = path.to_str().unwrap().as_bytes();
    assert!(name.len() < 108, "own AF_UNIX path must fit the OS limit");
    let mut data: WSADATA = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { WSAStartup(0x202, &mut data) }, 0);
    let raw = unsafe { socket(1, 1, 0) }; // AF_UNIX, SOCK_STREAM, default protocol.
    if raw == usize::MAX {
        let error = unsafe { WSAGetLastError() };
        unsafe { WSACleanup(); }
        panic!("own socket creation: {error}");
    }
    let owned = OwnSocket(raw);
    let mut address = Address { family: 1, path: [0; 108] };
    address.path[..name.len()].copy_from_slice(name);
    let result = unsafe { bind(raw, &address as *const Address as *const SOCKADDR, std::mem::size_of::<Address>() as i32) };
    assert_eq!(result, 0, "bind own socket: {}", unsafe { WSAGetLastError() });
    owned
}

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!("od-rt-{}", uuid::Uuid::new_v4().simple()));
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("in")).unwrap();
    fs::create_dir(root.join("out")).unwrap();
    root.canonicalize().unwrap()
}

#[test]
fn actual_windows_socket_leaf_is_accepted_inside_owned_parent() {
    let root = fixture();
    // Winsock paths do not accept the canonical extended-length prefix.
    let path = PathBuf::from(root.to_str().unwrap().strip_prefix(r"\\?\").unwrap_or(root.to_str().unwrap())).join("in/s");
    let socket = own_socket(&path);
    assert_eq!(path.canonicalize().unwrap_err().raw_os_error(), Some(1920), "exercise the actual OS failure boundary");
    assert!(observed_path(&path, &root.join("in")).is_ok(), "valid own AF_UNIX endpoint must not fail canonicalization");
    drop(socket);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn actual_windows_socket_under_foreign_junction_is_rejected() {
    let root = fixture();
    let short_root = PathBuf::from(root.to_str().unwrap().strip_prefix(r"\\?\").unwrap_or(root.to_str().unwrap()));
    let socket = own_socket(&short_root.join("out/s"));
    // cmd's mklink parses a forward slash as a switch, unlike filesystem APIs.
    let link = short_root.join("in").join("link");
    let output = std::process::Command::new("cmd.exe").args(["/D", "/C", "mklink", "/J"])
        .arg(&link).arg(short_root.join("out")).output().unwrap();
    assert!(output.status.success(), "create own junction pointing outside allowed parent: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let escaped = observed_path(&link.join("s"), &root.join("in"));
    let outside = observed_path(&short_root.join("out/s"), &root.join("in"));
    let traversal = observed_path(&short_root.join("in/../out/s"), &root.join("in"));
    fs::remove_dir(&link).unwrap();
    drop(socket);
    assert!(escaped.is_err() && outside.is_err() && traversal.is_err(), "socket handling must preserve owned parent containment");
    assert!(observed_path(&short_root.join("in/missing/s"), &root.join("in")).is_ok());
    fs::remove_dir_all(root).unwrap();
}
