// ABOUTME: Socket discovery and lock file management for MCP socket pooling
//
// Handles multi-instance coordination for sharing MCP server sockets:
// - LockFile: Advisory file locking with PID tracking for exclusive socket ownership
// - SocketDiscovery: Finding, validating, and cleaning up sockets in the pool directory
// - TOCTOU race prevention through atomic lock acquisition

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;
use tracing::{debug, info, warn};

use super::config::PoolConfig;

/// Errors that can occur during discovery and locking operations
#[derive(Error, Debug)]
pub enum DiscoveryError {
    /// Lock file is held by another process
    #[error("Lock held by another process (PID: {0})")]
    LockHeldByOther(u32),

    /// Failed to acquire lock (generic I/O error)
    #[error("Failed to acquire lock: {0}")]
    LockAcquisitionFailed(#[source] io::Error),

    /// Failed to release lock
    #[error("Failed to release lock: {0}")]
    LockReleaseFailed(#[source] io::Error),

    /// Invalid lock file content
    #[error("Invalid lock file content: {0}")]
    InvalidLockFile(String),

    /// Socket directory access failed
    #[error("Failed to access socket directory: {0}")]
    SocketDirAccessFailed(#[source] io::Error),

    /// Socket cleanup failed
    #[error("Failed to clean up stale socket: {0}")]
    CleanupFailed(#[source] io::Error),
}

/// Result type for discovery operations
pub type DiscoveryResult<T> = Result<T, DiscoveryError>;

/// Information about a discovered socket
#[derive(Debug, Clone)]
pub struct DiscoveredSocket {
    /// Path to the socket file
    pub socket_path: PathBuf,

    /// Path to the associated lock file
    pub lock_path: PathBuf,

    /// MCP server name extracted from socket filename
    pub mcp_name: String,

    /// PID of the process owning the socket (if lock file exists)
    pub owner_pid: Option<u32>,

    /// Whether the owning process is still alive
    pub is_alive: bool,
}

/// Advisory file lock with PID tracking
///
/// Provides exclusive access to a resource (typically an MCP socket) using
/// advisory file locking. The lock file contains the PID of the holding process
/// for debugging and stale lock detection.
///
/// # Lock Protocol
///
/// 1. Open lock file with `O_CREAT | O_RDWR`
/// 2. Acquire exclusive flock
/// 3. Write PID to file
/// 4. Resource is now exclusively owned
/// 5. On Drop: release lock and remove file
pub struct LockFile {
    /// Path to the lock file
    path: PathBuf,

    /// Open file handle (holds the lock)
    file: Option<File>,

    /// Whether this instance acquired the lock
    acquired: bool,
}

impl LockFile {
    /// Create a new lock file handle (does not acquire the lock)
    ///
    /// Use `acquire()` to actually obtain the lock.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            file: None,
            acquired: false,
        }
    }

    /// Attempt to acquire an exclusive lock
    ///
    /// # TOCTOU Prevention
    ///
    /// This uses flock which is atomic - the file open and lock acquisition
    /// happen as a single operation from the kernel's perspective.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if lock was acquired
    /// - `Ok(false)` if lock is held by another process (non-blocking)
    /// - `Err(...)` for I/O errors
    pub fn try_acquire(&mut self) -> DiscoveryResult<bool> {
        if self.acquired {
            return Ok(true);
        }

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(DiscoveryError::LockAcquisitionFailed)?;
        }

        // Open the file, creating if it doesn't exist
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .map_err(DiscoveryError::LockAcquisitionFailed)?;

        // Try to acquire exclusive lock (non-blocking)
        #[cfg(unix)]
        {
            use nix::fcntl::{FlockArg, flock};
            use std::os::unix::io::AsRawFd;

            let fd = file.as_raw_fd();
            match flock(fd, FlockArg::LockExclusiveNonblock) {
                Ok(()) => {
                    // Lock acquired successfully
                }
                Err(e) if e == nix::errno::Errno::EWOULDBLOCK || e == nix::errno::Errno::EAGAIN => {
                    // Lock held by another process
                    debug!(path = %self.path.display(), "Lock held by another process");
                    return Ok(false);
                }
                Err(e) => {
                    return Err(DiscoveryError::LockAcquisitionFailed(
                        io::Error::from_raw_os_error(e as i32),
                    ));
                }
            }
        }

        #[cfg(not(unix))]
        {
            // On non-Unix, we can't use flock - fall back to file existence check
            // This is less robust but better than nothing
            if self.path.exists() {
                return Ok(false);
            }
        }

        // Lock acquired - write our PID
        Self::write_pid(&file)?;
        self.file = Some(file);
        self.acquired = true;

        info!(path = %self.path.display(), pid = std::process::id(), "Lock acquired");
        Ok(true)
    }

    /// Acquire the lock, blocking until available
    ///
    /// This will wait indefinitely until the lock can be acquired.
    pub fn acquire(&mut self) -> DiscoveryResult<()> {
        if self.acquired {
            return Ok(());
        }

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(DiscoveryError::LockAcquisitionFailed)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .map_err(DiscoveryError::LockAcquisitionFailed)?;

        // Acquire exclusive lock (blocking)
        #[cfg(unix)]
        {
            use nix::fcntl::{FlockArg, flock};
            use std::os::unix::io::AsRawFd;

            let fd = file.as_raw_fd();
            flock(fd, FlockArg::LockExclusive).map_err(|e| {
                DiscoveryError::LockAcquisitionFailed(io::Error::from_raw_os_error(e as i32))
            })?;
        }

        // Write our PID
        Self::write_pid(&file)?;
        self.file = Some(file);
        self.acquired = true;

        info!(path = %self.path.display(), pid = std::process::id(), "Lock acquired (blocking)");
        Ok(())
    }

    /// Release the lock and remove the lock file
    pub fn release(&mut self) -> DiscoveryResult<()> {
        if !self.acquired {
            return Ok(());
        }

        // Release the flock
        #[cfg(unix)]
        if let Some(ref file) = self.file {
            use nix::fcntl::{FlockArg, flock};
            use std::os::unix::io::AsRawFd;

            let fd = file.as_raw_fd();
            if let Err(e) = flock(fd, FlockArg::Unlock) {
                warn!(
                    path = %self.path.display(),
                    error = %e,
                    "Failed to release flock"
                );
            }
        }

        // Close file handle
        self.file = None;

        // Remove the lock file
        if let Err(e) = fs::remove_file(&self.path) {
            if e.kind() != io::ErrorKind::NotFound {
                warn!(path = %self.path.display(), error = %e, "Failed to remove lock file");
                return Err(DiscoveryError::LockReleaseFailed(e));
            }
        }

        self.acquired = false;
        debug!(path = %self.path.display(), "Lock released");
        Ok(())
    }

    /// Check if this instance holds the lock
    #[must_use]
    pub const fn is_acquired(&self) -> bool {
        self.acquired
    }

    /// Read the PID from a lock file (without acquiring the lock)
    ///
    /// Useful for checking who owns a lock.
    pub fn read_owner_pid(path: &Path) -> Option<u32> {
        let mut file = File::open(path).ok()?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).ok()?;
        contents.trim().parse().ok()
    }

    /// Write our PID to the lock file
    fn write_pid(file: &File) -> DiscoveryResult<()> {
        // Clone the file handle so we can write without consuming
        let mut writer = file.try_clone().map_err(DiscoveryError::LockAcquisitionFailed)?;

        // Truncate and write PID
        writer.set_len(0).map_err(DiscoveryError::LockAcquisitionFailed)?;
        write!(writer, "{}", std::process::id()).map_err(DiscoveryError::LockAcquisitionFailed)?;
        writer.flush().map_err(DiscoveryError::LockAcquisitionFailed)?;

        Ok(())
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        if self.acquired {
            if let Err(e) = self.release() {
                warn!(
                    path = %self.path.display(),
                    error = %e,
                    "Failed to release lock in Drop"
                );
            }
        }
    }
}

/// Socket discovery and lifecycle management
///
/// Discovers existing MCP sockets in the pool directory, validates their
/// liveness by checking if the owning process is still running, and cleans
/// up stale sockets from dead processes.
pub struct SocketDiscovery {
    /// Pool configuration
    config: PoolConfig,
}

impl SocketDiscovery {
    /// Create a new socket discovery instance
    #[must_use]
    pub const fn new(config: PoolConfig) -> Self {
        Self { config }
    }

    /// Discover all sockets in the pool directory
    ///
    /// Returns information about each socket including:
    /// - Socket and lock file paths
    /// - MCP server name
    /// - Owner PID (if available)
    /// - Whether the owner is still alive
    pub fn discover_all(&self) -> DiscoveryResult<Vec<DiscoveredSocket>> {
        let socket_dir =
            self.config.get_socket_dir().map_err(DiscoveryError::SocketDirAccessFailed)?;

        let mut sockets = Vec::new();

        // Read directory entries
        let entries = match fs::read_dir(&socket_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // Directory doesn't exist yet - no sockets
                return Ok(Vec::new());
            }
            Err(e) => return Err(DiscoveryError::SocketDirAccessFailed(e)),
        };

        // Pattern: mcp-{name}.sock
        let prefix = &self.config.socket_prefix;

        for entry in entries.flatten() {
            let path = entry.path();

            // Only process socket files
            let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            if !filename.starts_with(prefix) || !filename.ends_with(".sock") {
                continue;
            }

            // Extract MCP name: mcp-{name}.sock -> {name}
            let name_start = prefix.len();
            let name_end = filename.len() - 5; // ".sock".len()
            if name_start >= name_end {
                continue;
            }

            let mcp_name = filename[name_start..name_end].to_string();
            let lock_path = socket_dir.join(format!("{prefix}{mcp_name}.lock"));

            // Read owner PID from lock file
            let owner_pid = LockFile::read_owner_pid(&lock_path);

            // Check if owner is alive
            let is_alive = owner_pid.is_some_and(Self::is_process_alive);

            sockets.push(DiscoveredSocket {
                socket_path: path,
                lock_path,
                mcp_name,
                owner_pid,
                is_alive,
            });
        }

        debug!(
            socket_dir = %socket_dir.display(),
            count = sockets.len(),
            "Discovered sockets"
        );

        Ok(sockets)
    }

    /// Find a specific socket by MCP name
    pub fn find_socket(&self, mcp_name: &str) -> DiscoveryResult<Option<DiscoveredSocket>> {
        let socket_path = self
            .config
            .get_socket_path(mcp_name)
            .map_err(DiscoveryError::SocketDirAccessFailed)?;

        let lock_path = self
            .config
            .get_lock_path(mcp_name)
            .map_err(DiscoveryError::SocketDirAccessFailed)?;

        // Check if socket exists
        if !socket_path.exists() {
            return Ok(None);
        }

        let owner_pid = LockFile::read_owner_pid(&lock_path);
        let is_alive = owner_pid.is_some_and(Self::is_process_alive);

        Ok(Some(DiscoveredSocket {
            socket_path,
            lock_path,
            mcp_name: mcp_name.to_string(),
            owner_pid,
            is_alive,
        }))
    }

    /// Clean up stale sockets from dead processes
    ///
    /// Removes socket and lock files where the owning process is no longer running.
    /// Returns the count of cleaned up sockets.
    pub fn cleanup_stale(&self) -> DiscoveryResult<usize> {
        let sockets = self.discover_all()?;
        let mut cleaned = 0;

        for socket in sockets {
            // Only clean up if:
            // 1. We know the owner PID (lock file exists and is readable)
            // 2. The owner process is dead
            if socket.owner_pid.is_some() && !socket.is_alive {
                debug!(
                    mcp = %socket.mcp_name,
                    pid = ?socket.owner_pid,
                    "Cleaning up stale socket"
                );

                // Remove socket file
                if let Err(e) = fs::remove_file(&socket.socket_path) {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!(
                            path = %socket.socket_path.display(),
                            error = %e,
                            "Failed to remove stale socket"
                        );
                    }
                }

                // Remove lock file
                if let Err(e) = fs::remove_file(&socket.lock_path) {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!(
                            path = %socket.lock_path.display(),
                            error = %e,
                            "Failed to remove stale lock"
                        );
                    }
                }

                cleaned += 1;
            }
        }

        if cleaned > 0 {
            info!(count = cleaned, "Cleaned up stale sockets");
        }

        Ok(cleaned)
    }

    /// Clean up a specific socket (used when releasing ownership)
    pub fn cleanup_socket(&self, mcp_name: &str) -> DiscoveryResult<()> {
        let socket_path = self
            .config
            .get_socket_path(mcp_name)
            .map_err(DiscoveryError::SocketDirAccessFailed)?;

        let lock_path = self
            .config
            .get_lock_path(mcp_name)
            .map_err(DiscoveryError::SocketDirAccessFailed)?;

        // Remove socket file
        if let Err(e) = fs::remove_file(&socket_path) {
            if e.kind() != io::ErrorKind::NotFound {
                return Err(DiscoveryError::CleanupFailed(e));
            }
        }

        // Remove lock file
        if let Err(e) = fs::remove_file(&lock_path) {
            if e.kind() != io::ErrorKind::NotFound {
                return Err(DiscoveryError::CleanupFailed(e));
            }
        }

        debug!(mcp = %mcp_name, "Socket cleaned up");
        Ok(())
    }

    /// Check if a process with the given PID is alive
    ///
    /// Uses `kill(pid, 0)` which checks if we can send a signal to the process
    /// without actually sending anything. Returns true if the process exists.
    #[cfg(unix)]
    fn is_process_alive(pid: u32) -> bool {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        // Safe cast: typical PIDs fit in i32
        #[allow(clippy::cast_possible_wrap)]
        let nix_pid = Pid::from_raw(pid as i32);

        // kill with signal 0 doesn't send a signal, just checks if process exists
        // and we have permission to signal it
        kill(nix_pid, None).is_ok()
    }

    #[cfg(not(unix))]
    fn is_process_alive(_pid: u32) -> bool {
        // On non-Unix platforms, we can't easily check process liveness
        // Assume alive to be safe (avoid accidentally cleaning up active sockets)
        true
    }

    /// Get live sockets that are ready to accept connections
    pub fn get_live_sockets(&self) -> DiscoveryResult<Vec<DiscoveredSocket>> {
        let all = self.discover_all()?;
        Ok(all.into_iter().filter(|s| s.is_alive).collect())
    }

    /// Get stale sockets that need cleanup
    pub fn get_stale_sockets(&self) -> DiscoveryResult<Vec<DiscoveredSocket>> {
        let all = self.discover_all()?;
        Ok(all.into_iter().filter(|s| s.owner_pid.is_some() && !s.is_alive).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tempfile::TempDir;

    // Unique counter for test isolation
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn test_config(temp_dir: &TempDir) -> PoolConfig {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let socket_dir = temp_dir.path().join(format!("sockets_{counter}"));
        fs::create_dir_all(&socket_dir).unwrap();

        PoolConfig {
            socket_dir: Some(socket_dir),
            socket_prefix: "mcp-".to_string(),
            ..PoolConfig::default()
        }
    }

    // ==================== LockFile Tests ====================

    #[test]
    fn test_lockfile_new() {
        let path = PathBuf::from("/tmp/test.lock");
        let lock = LockFile::new(path.clone());

        assert_eq!(lock.path, path);
        assert!(!lock.acquired);
        assert!(lock.file.is_none());
    }

    #[test]
    fn test_lockfile_try_acquire_success() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        let mut lock = LockFile::new(lock_path.clone());
        let result = lock.try_acquire();

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(lock.is_acquired());
        assert!(lock_path.exists());

        // Verify PID was written
        let contents = fs::read_to_string(&lock_path).unwrap();
        assert_eq!(contents, std::process::id().to_string());
    }

    #[test]
    fn test_lockfile_try_acquire_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        let mut lock = LockFile::new(lock_path);

        // First acquire
        assert!(lock.try_acquire().unwrap());

        // Second acquire should also succeed (already held)
        assert!(lock.try_acquire().unwrap());
        assert!(lock.is_acquired());
    }

    #[test]
    fn test_lockfile_release() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        let mut lock = LockFile::new(lock_path.clone());
        lock.try_acquire().unwrap();
        assert!(lock_path.exists());

        // Release
        let result = lock.release();
        assert!(result.is_ok());
        assert!(!lock.is_acquired());
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_lockfile_release_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        let mut lock = LockFile::new(lock_path);

        // Release without acquiring should be ok
        assert!(lock.release().is_ok());

        // Acquire then release twice should be ok
        lock.try_acquire().unwrap();
        assert!(lock.release().is_ok());
        assert!(lock.release().is_ok());
    }

    #[test]
    fn test_lockfile_drop_releases() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        {
            let mut lock = LockFile::new(lock_path.clone());
            lock.try_acquire().unwrap();
            assert!(lock_path.exists());
            // lock goes out of scope here
        }

        // File should be removed after Drop
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_lockfile_read_owner_pid() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        // Write a known PID
        fs::write(&lock_path, "12345").unwrap();

        let pid = LockFile::read_owner_pid(&lock_path);
        assert_eq!(pid, Some(12345));
    }

    #[test]
    fn test_lockfile_read_owner_pid_nonexistent() {
        let path = PathBuf::from("/nonexistent/path/test.lock");
        let pid = LockFile::read_owner_pid(&path);
        assert!(pid.is_none());
    }

    #[test]
    fn test_lockfile_read_owner_pid_invalid() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        // Write invalid content
        fs::write(&lock_path, "not-a-number").unwrap();

        let pid = LockFile::read_owner_pid(&lock_path);
        assert!(pid.is_none());
    }

    #[test]
    fn test_lockfile_creates_parent_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("nested").join("dirs").join("test.lock");

        let mut lock = LockFile::new(lock_path.clone());
        let result = lock.try_acquire();

        assert!(result.is_ok());
        assert!(lock_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_lockfile_concurrent_lock_blocked() {
        use std::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        // Acquire lock in this process
        let mut lock = LockFile::new(lock_path.clone());
        assert!(lock.try_acquire().unwrap());

        // Try to acquire in a child process using flock command
        let output = Command::new("flock")
            .args(["-n", lock_path.to_str().unwrap(), "-c", "echo locked"])
            .output();

        // If flock command exists, it should fail because we hold the lock
        if let Ok(output) = output {
            // The command should fail (exit non-zero) because lock is held
            // Note: If flock isn't installed, we skip this assertion
            if output.status.success() {
                panic!("Child process acquired lock when it shouldn't have");
            }
        }
    }

    #[test]
    fn test_lockfile_blocking_acquire() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("test.lock");

        let mut lock = LockFile::new(lock_path.clone());
        let result = lock.acquire();

        assert!(result.is_ok());
        assert!(lock.is_acquired());
        assert!(lock_path.exists());
    }

    // ==================== SocketDiscovery Tests ====================

    #[test]
    fn test_discovery_new() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let discovery = SocketDiscovery::new(config);
        assert!(!discovery.config.socket_prefix.is_empty());
    }

    #[test]
    fn test_discovery_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let discovery = SocketDiscovery::new(config);

        let result = discovery.discover_all();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_discovery_finds_sockets() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Create some socket files
        fs::write(socket_dir.join("mcp-context7.sock"), "").unwrap();
        fs::write(socket_dir.join("mcp-memory.sock"), "").unwrap();
        fs::write(socket_dir.join("other.txt"), "").unwrap(); // Should be ignored

        let discovery = SocketDiscovery::new(config);
        let result = discovery.discover_all().unwrap();

        assert_eq!(result.len(), 2);
        let names: Vec<_> = result.iter().map(|s| &s.mcp_name).collect();
        assert!(names.contains(&&"context7".to_string()));
        assert!(names.contains(&&"memory".to_string()));
    }

    #[test]
    fn test_discovery_reads_owner_pid() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Create socket and lock files
        fs::write(socket_dir.join("mcp-test.sock"), "").unwrap();
        fs::write(socket_dir.join("mcp-test.lock"), "42").unwrap();

        let discovery = SocketDiscovery::new(config);
        let result = discovery.discover_all().unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].owner_pid, Some(42));
    }

    #[test]
    fn test_discovery_find_socket_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Create socket file
        fs::write(socket_dir.join("mcp-context7.sock"), "").unwrap();

        let discovery = SocketDiscovery::new(config);
        let result = discovery.find_socket("context7").unwrap();

        assert!(result.is_some());
        let socket = result.unwrap();
        assert_eq!(socket.mcp_name, "context7");
    }

    #[test]
    fn test_discovery_find_socket_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let discovery = SocketDiscovery::new(config);

        let result = discovery.find_socket("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_discovery_cleanup_stale() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Create socket with dead PID (assuming PID 999999999 doesn't exist)
        fs::write(socket_dir.join("mcp-stale.sock"), "").unwrap();
        fs::write(socket_dir.join("mcp-stale.lock"), "999999999").unwrap();

        let discovery = SocketDiscovery::new(config);
        let cleaned = discovery.cleanup_stale().unwrap();

        assert_eq!(cleaned, 1);
        assert!(!socket_dir.join("mcp-stale.sock").exists());
        assert!(!socket_dir.join("mcp-stale.lock").exists());
    }

    #[test]
    fn test_discovery_cleanup_preserves_live() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Create socket with our own PID (definitely alive)
        let our_pid = std::process::id();
        fs::write(socket_dir.join("mcp-live.sock"), "").unwrap();
        fs::write(socket_dir.join("mcp-live.lock"), our_pid.to_string()).unwrap();

        let discovery = SocketDiscovery::new(config);
        let cleaned = discovery.cleanup_stale().unwrap();

        assert_eq!(cleaned, 0);
        assert!(socket_dir.join("mcp-live.sock").exists());
        assert!(socket_dir.join("mcp-live.lock").exists());
    }

    #[test]
    fn test_discovery_cleanup_socket() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Create socket and lock
        fs::write(socket_dir.join("mcp-test.sock"), "").unwrap();
        fs::write(socket_dir.join("mcp-test.lock"), "123").unwrap();

        let discovery = SocketDiscovery::new(config);
        let result = discovery.cleanup_socket("test");

        assert!(result.is_ok());
        assert!(!socket_dir.join("mcp-test.sock").exists());
        assert!(!socket_dir.join("mcp-test.lock").exists());
    }

    #[test]
    fn test_discovery_cleanup_socket_not_exists_ok() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let discovery = SocketDiscovery::new(config);

        // Should not error on nonexistent socket
        let result = discovery.cleanup_socket("nonexistent");
        assert!(result.is_ok());
    }

    #[test]
    fn test_discovery_get_live_sockets() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Create live socket (our PID)
        let our_pid = std::process::id();
        fs::write(socket_dir.join("mcp-live.sock"), "").unwrap();
        fs::write(socket_dir.join("mcp-live.lock"), our_pid.to_string()).unwrap();

        // Create stale socket (dead PID)
        fs::write(socket_dir.join("mcp-stale.sock"), "").unwrap();
        fs::write(socket_dir.join("mcp-stale.lock"), "999999999").unwrap();

        let discovery = SocketDiscovery::new(config);
        let live = discovery.get_live_sockets().unwrap();

        assert_eq!(live.len(), 1);
        assert_eq!(live[0].mcp_name, "live");
    }

    #[test]
    fn test_discovery_get_stale_sockets() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Create live socket
        let our_pid = std::process::id();
        fs::write(socket_dir.join("mcp-live.sock"), "").unwrap();
        fs::write(socket_dir.join("mcp-live.lock"), our_pid.to_string()).unwrap();

        // Create stale socket
        fs::write(socket_dir.join("mcp-stale.sock"), "").unwrap();
        fs::write(socket_dir.join("mcp-stale.lock"), "999999999").unwrap();

        let discovery = SocketDiscovery::new(config);
        let stale = discovery.get_stale_sockets().unwrap();

        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].mcp_name, "stale");
    }

    #[cfg(unix)]
    #[test]
    fn test_is_process_alive_self() {
        // Our own process should be alive
        let our_pid = std::process::id();
        assert!(SocketDiscovery::is_process_alive(our_pid));
    }

    #[cfg(unix)]
    #[test]
    fn test_is_process_alive_parent() {
        // Parent process should always be alive (since we're running)
        // Note: PID 1 (init/launchd) may not be checkable without root permissions
        use nix::unistd::getppid;
        let parent_pid = getppid().as_raw() as u32;
        assert!(SocketDiscovery::is_process_alive(parent_pid));
    }

    #[cfg(unix)]
    #[test]
    fn test_is_process_alive_dead() {
        // Very high PID that likely doesn't exist
        assert!(!SocketDiscovery::is_process_alive(999_999_999));
    }

    // ==================== DiscoveredSocket Tests ====================

    #[test]
    fn test_discovered_socket_fields() {
        let socket = DiscoveredSocket {
            socket_path: PathBuf::from("/tmp/mcp-test.sock"),
            lock_path: PathBuf::from("/tmp/mcp-test.lock"),
            mcp_name: "test".to_string(),
            owner_pid: Some(123),
            is_alive: true,
        };

        assert_eq!(socket.mcp_name, "test");
        assert_eq!(socket.owner_pid, Some(123));
        assert!(socket.is_alive);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_lock_then_discover() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Acquire lock
        let lock_path = socket_dir.join("mcp-test.lock");
        let mut lock = LockFile::new(lock_path);
        lock.try_acquire().unwrap();

        // Create socket file
        fs::write(socket_dir.join("mcp-test.sock"), "").unwrap();

        // Discover should find it with our PID
        let discovery = SocketDiscovery::new(config);
        let sockets = discovery.discover_all().unwrap();

        assert_eq!(sockets.len(), 1);
        assert_eq!(sockets[0].mcp_name, "test");
        assert_eq!(sockets[0].owner_pid, Some(std::process::id()));
        assert!(sockets[0].is_alive);
    }

    #[test]
    fn test_full_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // 1. Start with empty directory
        let discovery = SocketDiscovery::new(config.clone());
        assert!(discovery.discover_all().unwrap().is_empty());

        // 2. Acquire lock and create socket
        let lock_path = config.get_lock_path("test").unwrap();
        let mut lock = LockFile::new(lock_path);
        lock.try_acquire().unwrap();

        let socket_path = config.get_socket_path("test").unwrap();
        fs::write(&socket_path, "").unwrap();

        // 3. Verify discovery finds it
        let sockets = discovery.discover_all().unwrap();
        assert_eq!(sockets.len(), 1);
        assert!(sockets[0].is_alive);

        // 4. Release lock
        lock.release().unwrap();

        // 5. Socket still exists but lock is gone
        assert!(socket_path.exists());
        assert!(!config.get_lock_path("test").unwrap().exists());

        // 6. Cleanup socket
        discovery.cleanup_socket("test").unwrap();
        assert!(!socket_path.exists());
    }
}
