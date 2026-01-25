// ABOUTME: MCP process supervision with restart logic and exponential backoff
//
// Manages MCP server processes with lifecycle management including spawn, monitor,
// restart with exponential backoff, and graceful termination. Handles SIGCHLD via
// tokio's built-in signal handling and prevents zombie processes.

// Allow if-let/else patterns instead of map_or_else for readability
// Allow non-const take functions (Option::take is not const-stable)
#![allow(clippy::option_if_let_else)]
#![allow(clippy::missing_const_for_fn)]

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use thiserror::Error;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::{debug, error, info, warn};

/// Errors that can occur during process supervision
#[derive(Error, Debug)]
pub enum SupervisorError {
    /// Process spawn failed
    #[error("Failed to spawn process: {0}")]
    SpawnFailed(#[source] std::io::Error),

    /// Process already running
    #[error("Process already running with PID {0}")]
    AlreadyRunning(u32),

    /// Process not running when expected
    #[error("Process not running")]
    NotRunning,

    /// Maximum restarts exceeded
    #[error("Maximum restarts ({0}) exceeded")]
    MaxRestartsExceeded(u32),

    /// Termination failed
    #[error("Failed to terminate process: {0}")]
    TerminateFailed(#[source] std::io::Error),

    /// Invalid command
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
}

/// Current state of a supervised process
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessState {
    /// Process has not been started yet
    NotStarted,

    /// Process is running with the given PID
    Running {
        /// Process ID
        pid: u32,
    },

    /// Process exited normally with exit code
    Exited {
        /// Exit code from the process
        code: i32,
    },

    /// Process was terminated by a signal
    Signaled {
        /// Signal number that terminated the process
        signal: i32,
    },

    /// Process failed to start or run
    Failed {
        /// Description of the failure
        reason: String,
    },
}

impl ProcessState {
    /// Returns true if the process is currently running
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

/// Exponential backoff calculator for restart delays
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    /// Base delay duration
    base: Duration,

    /// Maximum delay cap
    max: Duration,

    /// Current attempt number (0-indexed)
    current_attempt: u32,
}

impl ExponentialBackoff {
    /// Create a new exponential backoff calculator
    ///
    /// # Arguments
    /// * `base` - Base delay duration (first retry delay)
    /// * `max` - Maximum delay cap
    #[must_use]
    pub const fn new(base: Duration, max: Duration) -> Self {
        Self {
            base,
            max,
            current_attempt: 0,
        }
    }

    /// Calculate the next delay and increment the attempt counter
    ///
    /// Returns `min(base * 2^attempt, max)`
    pub fn next_delay(&mut self) -> Duration {
        // Use saturating arithmetic to prevent overflow
        let multiplier = 2u64.saturating_pow(self.current_attempt);
        // Truncate to u64 - millis > u64::MAX is already capped by max duration
        let base_millis = u64::try_from(self.base.as_millis()).unwrap_or(u64::MAX);

        // Saturating multiply to handle overflow
        let delay_millis = base_millis.saturating_mul(multiplier);
        let delay = Duration::from_millis(delay_millis);

        self.current_attempt = self.current_attempt.saturating_add(1);

        std::cmp::min(delay, self.max)
    }

    /// Reset the attempt counter to 0
    pub const fn reset(&mut self) {
        self.current_attempt = 0;
    }

    /// Get the current attempt number
    #[must_use]
    pub const fn current_attempt(&self) -> u32 {
        self.current_attempt
    }
}

/// Process supervisor managing the lifecycle of a single MCP server process
pub struct ProcessSupervisor {
    /// The child process (if running)
    child: Option<Child>,

    /// Child's stdin for JSON-RPC communication
    stdin: Option<ChildStdin>,

    /// Child's stdout for JSON-RPC communication
    stdout: Option<ChildStdout>,

    /// Current process state
    state: ProcessState,

    /// Number of times this process has been restarted
    restart_count: u32,

    /// Maximum allowed restarts before permanent failure
    max_restarts: u32,

    /// Backoff calculator for restart delays
    backoff: ExponentialBackoff,

    /// Timestamp of last restart attempt
    last_restart: Option<Instant>,
}

impl ProcessSupervisor {
    /// Create a new process supervisor
    ///
    /// # Arguments
    /// * `max_restarts` - Maximum number of restart attempts before permanent failure
    /// * `backoff_base` - Base duration for exponential backoff
    /// * `backoff_max` - Maximum duration cap for exponential backoff
    #[must_use]
    pub const fn new(max_restarts: u32, backoff_base: Duration, backoff_max: Duration) -> Self {
        Self {
            child: None,
            stdin: None,
            stdout: None,
            state: ProcessState::NotStarted,
            restart_count: 0,
            max_restarts,
            backoff: ExponentialBackoff::new(backoff_base, backoff_max),
            last_restart: None,
        }
    }

    /// Spawn a new process
    ///
    /// # Arguments
    /// * `command` - The command to execute
    /// * `args` - Command arguments
    /// * `env` - Environment variables to set
    /// * `cwd` - Optional working directory
    ///
    /// # Errors
    /// Returns error if process is already running or spawn fails
    #[allow(clippy::unused_async)] // async for API consistency with other supervisor methods
    pub async fn spawn(
        &mut self,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&Path>,
    ) -> Result<(), SupervisorError> {
        // Check if already running
        if let ProcessState::Running { pid } = self.state {
            return Err(SupervisorError::AlreadyRunning(pid));
        }

        // Validate command
        if command.is_empty() {
            return Err(SupervisorError::InvalidCommand(
                "Command cannot be empty".to_string(),
            ));
        }

        debug!(command = %command, args = ?args, "Spawning MCP process");

        // Build the command
        let mut process_cmd = Command::new(command);
        process_cmd
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);

        // Set environment variables
        for (key, value) in env {
            process_cmd.env(key, value);
        }

        // Set working directory if provided
        if let Some(dir) = cwd {
            process_cmd.current_dir(dir);
        }

        // Spawn the process
        let mut child = process_cmd.spawn().map_err(SupervisorError::SpawnFailed)?;

        // Extract stdio handles
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();

        // Get PID
        let pid = child.id().unwrap_or(0);

        info!(pid = pid, command = %command, "MCP process spawned");

        // Update state
        self.child = Some(child);
        self.stdin = stdin;
        self.stdout = stdout;
        self.state = ProcessState::Running { pid };

        Ok(())
    }

    /// Check the current status of the process
    ///
    /// This polls the child process to see if it has exited and updates the state accordingly.
    /// Returns a clone of the current state.
    pub fn check_status(&mut self) -> ProcessState {
        if let Some(ref mut child) = self.child {
            // Try to get the exit status without blocking
            match child.try_wait() {
                Ok(Some(status)) => {
                    let new_state = if let Some(code) = status.code() {
                        debug!(code = code, "Process exited with code");
                        ProcessState::Exited { code }
                    } else {
                        // Process was terminated by signal
                        #[cfg(unix)]
                        {
                            use std::os::unix::process::ExitStatusExt;
                            if let Some(signal) = status.signal() {
                                debug!(signal = signal, "Process terminated by signal");
                                ProcessState::Signaled { signal }
                            } else {
                                ProcessState::Failed {
                                    reason: "Unknown termination".to_string(),
                                }
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            ProcessState::Failed {
                                reason: "Unknown termination".to_string(),
                            }
                        }
                    };

                    self.state = new_state;
                    self.child = None;
                    self.stdin = None;
                    self.stdout = None;
                }
                Ok(None) => {
                    // Process is still running
                }
                Err(e) => {
                    error!(error = %e, "Failed to check process status");
                    self.state = ProcessState::Failed {
                        reason: format!("Status check failed: {e}"),
                    };
                }
            }
        }

        self.state.clone()
    }

    /// Restart the process with exponential backoff
    ///
    /// # Arguments
    /// * `command` - The command to execute
    /// * `args` - Command arguments
    /// * `env` - Environment variables to set
    /// * `cwd` - Optional working directory
    ///
    /// # Errors
    /// Returns error if max restarts exceeded or spawn fails
    pub async fn restart(
        &mut self,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&Path>,
    ) -> Result<(), SupervisorError> {
        // Check if we've exceeded max restarts
        if self.restart_count >= self.max_restarts {
            self.state = ProcessState::Failed {
                reason: format!("Exceeded maximum restarts ({})", self.max_restarts),
            };
            return Err(SupervisorError::MaxRestartsExceeded(self.max_restarts));
        }

        // Terminate existing process if running
        if self.state.is_running() {
            if let Err(e) = self.terminate().await {
                warn!(error = %e, "Failed to terminate process before restart");
            }
        }

        // Calculate backoff delay
        let delay = self.backoff.next_delay();
        info!(
            attempt = self.restart_count + 1,
            max = self.max_restarts,
            delay_ms = delay.as_millis(),
            "Restarting process with backoff"
        );

        // Wait for backoff duration
        tokio::time::sleep(delay).await;

        // Increment restart count
        self.restart_count += 1;
        self.last_restart = Some(Instant::now());

        // Spawn the process
        self.spawn(command, args, env, cwd).await
    }

    /// Gracefully terminate the process
    ///
    /// First sends SIGTERM, waits briefly, then sends SIGKILL if process hasn't exited.
    ///
    /// # Errors
    /// Returns error if termination fails
    pub async fn terminate(&mut self) -> Result<(), SupervisorError> {
        let Some(ref mut child) = self.child else {
            return Ok(());
        };

        let pid = child.id().unwrap_or(0);
        info!(pid = pid, "Terminating MCP process");

        // First, try graceful shutdown with SIGTERM
        #[cfg(unix)]
        {
            use nix::sys::signal::{Signal, kill};
            use nix::unistd::Pid;

            if pid > 0 {
                // Safe cast: pid is u32, fits in i32 for typical PIDs
                #[allow(clippy::cast_possible_wrap)]
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
            }
        }

        #[cfg(not(unix))]
        {
            // On non-Unix, just kill directly
            let _ = child.kill().await;
        }

        // Wait briefly for graceful shutdown
        let timeout = Duration::from_secs(2);
        let result = tokio::time::timeout(timeout, child.wait()).await;

        match result {
            Ok(Ok(_status)) => {
                debug!(pid = pid, "Process terminated gracefully");
            }
            Ok(Err(e)) => {
                warn!(error = %e, "Error waiting for process");
            }
            Err(_) => {
                // Timeout - force kill
                warn!(
                    pid = pid,
                    "Process did not terminate gracefully, sending SIGKILL"
                );

                if let Err(e) = child.kill().await {
                    error!(error = %e, "Failed to kill process");
                    return Err(SupervisorError::TerminateFailed(e));
                }

                // Wait for the kill to take effect
                let _ = child.wait().await;
            }
        }

        self.state = ProcessState::Exited { code: 0 };
        self.child = None;
        self.stdin = None;
        self.stdout = None;

        Ok(())
    }

    /// Take ownership of the child's stdin handle
    ///
    /// This can only be called once - subsequent calls return None.
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.stdin.take()
    }

    /// Take ownership of the child's stdout handle
    ///
    /// This can only be called once - subsequent calls return None.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.stdout.take()
    }

    /// Check if the process is currently running
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.state.is_running()
    }

    /// Check if the process has permanently failed (exceeded max restarts)
    #[must_use]
    pub const fn is_permanently_failed(&self) -> bool {
        self.restart_count >= self.max_restarts
            && matches!(
                self.state,
                ProcessState::Failed { .. }
                    | ProcessState::Exited { .. }
                    | ProcessState::Signaled { .. }
            )
    }

    /// Get the current restart count
    #[must_use]
    pub const fn restart_count(&self) -> u32 {
        self.restart_count
    }

    /// Get a reference to the current process state
    #[must_use]
    pub const fn state(&self) -> &ProcessState {
        &self.state
    }

    /// Get the timestamp of the last restart
    #[must_use]
    pub const fn last_restart(&self) -> Option<Instant> {
        self.last_restart
    }

    /// Reset the backoff counter (call after sustained successful operation)
    pub const fn reset_backoff(&mut self) {
        self.backoff.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ExponentialBackoff Tests ====================

    #[test]
    fn test_exponential_backoff_new() {
        let backoff = ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(60));

        assert_eq!(backoff.base, Duration::from_secs(1));
        assert_eq!(backoff.max, Duration::from_secs(60));
        assert_eq!(backoff.current_attempt, 0);
    }

    #[test]
    fn test_exponential_backoff_next_delay_sequence() {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(10));

        // First attempt: 100ms * 2^0 = 100ms
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));

        // Second attempt: 100ms * 2^1 = 200ms
        assert_eq!(backoff.next_delay(), Duration::from_millis(200));

        // Third attempt: 100ms * 2^2 = 400ms
        assert_eq!(backoff.next_delay(), Duration::from_millis(400));

        // Fourth attempt: 100ms * 2^3 = 800ms
        assert_eq!(backoff.next_delay(), Duration::from_millis(800));
    }

    #[test]
    fn test_exponential_backoff_respects_max() {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_millis(100), Duration::from_millis(500));

        // First few are under max
        assert_eq!(backoff.next_delay(), Duration::from_millis(100)); // 2^0 * 100
        assert_eq!(backoff.next_delay(), Duration::from_millis(200)); // 2^1 * 100
        assert_eq!(backoff.next_delay(), Duration::from_millis(400)); // 2^2 * 100

        // Next would be 800ms but capped at 500ms
        assert_eq!(backoff.next_delay(), Duration::from_millis(500));
        assert_eq!(backoff.next_delay(), Duration::from_millis(500));
    }

    #[test]
    fn test_exponential_backoff_reset() {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(10));

        // Advance a few times
        backoff.next_delay();
        backoff.next_delay();
        backoff.next_delay();
        assert_eq!(backoff.current_attempt, 3);

        // Reset
        backoff.reset();
        assert_eq!(backoff.current_attempt, 0);

        // Should start from beginning again
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn test_exponential_backoff_overflow_protection() {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(3600));

        // Run many iterations to test overflow protection
        for _ in 0..100 {
            let delay = backoff.next_delay();
            // Should never exceed max
            assert!(delay <= Duration::from_secs(3600));
            // Should never be zero
            assert!(delay > Duration::ZERO);
        }
    }

    // ==================== ProcessState Tests ====================

    #[test]
    fn test_process_state_is_running() {
        assert!(!ProcessState::NotStarted.is_running());
        assert!(ProcessState::Running { pid: 123 }.is_running());
        assert!(!ProcessState::Exited { code: 0 }.is_running());
        assert!(!ProcessState::Signaled { signal: 9 }.is_running());
        assert!(
            !ProcessState::Failed {
                reason: "test".to_string()
            }
            .is_running()
        );
    }

    #[test]
    fn test_process_state_equality() {
        assert_eq!(ProcessState::NotStarted, ProcessState::NotStarted);
        assert_eq!(
            ProcessState::Running { pid: 100 },
            ProcessState::Running { pid: 100 }
        );
        assert_ne!(
            ProcessState::Running { pid: 100 },
            ProcessState::Running { pid: 200 }
        );
        assert_eq!(
            ProcessState::Exited { code: -1 },
            ProcessState::Exited { code: -1 }
        );
    }

    // ==================== ProcessSupervisor Tests ====================

    #[test]
    fn test_supervisor_new() {
        let supervisor =
            ProcessSupervisor::new(10, Duration::from_secs(1), Duration::from_secs(60));

        assert_eq!(supervisor.max_restarts, 10);
        assert_eq!(supervisor.restart_count, 0);
        assert_eq!(*supervisor.state(), ProcessState::NotStarted);
        assert!(!supervisor.is_running());
        assert!(!supervisor.is_permanently_failed());
    }

    #[tokio::test]
    async fn test_supervisor_spawn_simple_command() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        // Spawn a simple command that exits quickly
        let result = supervisor.spawn("echo", &["hello".to_string()], &HashMap::new(), None).await;

        assert!(result.is_ok());
        assert!(supervisor.is_running());
        assert!(matches!(supervisor.state(), ProcessState::Running { .. }));

        // Should have stdin/stdout available
        assert!(supervisor.stdin.is_some());
        assert!(supervisor.stdout.is_some());

        // Terminate to clean up
        let _ = supervisor.terminate().await;
    }

    #[tokio::test]
    async fn test_supervisor_spawn_empty_command_fails() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        let result = supervisor.spawn("", &[], &HashMap::new(), None).await;

        assert!(matches!(result, Err(SupervisorError::InvalidCommand(_))));
    }

    #[tokio::test]
    async fn test_supervisor_spawn_invalid_command_fails() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        let result = supervisor
            .spawn(
                "/nonexistent/command/that/does/not/exist",
                &[],
                &HashMap::new(),
                None,
            )
            .await;

        assert!(matches!(result, Err(SupervisorError::SpawnFailed(_))));
    }

    #[tokio::test]
    async fn test_supervisor_spawn_already_running() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        // Spawn a long-running process (sleep)
        let result = supervisor.spawn("sleep", &["10".to_string()], &HashMap::new(), None).await;
        assert!(result.is_ok());

        // Try to spawn again while running
        let result2 = supervisor.spawn("echo", &["test".to_string()], &HashMap::new(), None).await;

        assert!(matches!(result2, Err(SupervisorError::AlreadyRunning(_))));

        // Cleanup
        let _ = supervisor.terminate().await;
    }

    #[tokio::test]
    async fn test_supervisor_check_status_detects_exit() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        // Spawn a command that exits immediately
        let result = supervisor.spawn("true", &[], &HashMap::new(), None).await;
        assert!(result.is_ok());

        // Give it time to exit
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check status should detect exit
        let state = supervisor.check_status();

        assert!(matches!(state, ProcessState::Exited { code: 0 }));
        assert!(!supervisor.is_running());
    }

    #[tokio::test]
    async fn test_supervisor_check_status_detects_nonzero_exit() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        // Spawn a command that exits with error
        let result = supervisor.spawn("false", &[], &HashMap::new(), None).await;
        assert!(result.is_ok());

        // Give it time to exit
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check status should detect non-zero exit
        let state = supervisor.check_status();

        assert!(matches!(state, ProcessState::Exited { code: 1 }));
    }

    #[tokio::test]
    async fn test_supervisor_terminate() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        // Spawn a long-running process
        let result = supervisor.spawn("sleep", &["30".to_string()], &HashMap::new(), None).await;
        assert!(result.is_ok());
        assert!(supervisor.is_running());

        // Terminate it
        let term_result = supervisor.terminate().await;
        assert!(term_result.is_ok());

        assert!(!supervisor.is_running());
    }

    #[tokio::test]
    async fn test_supervisor_terminate_not_running_is_ok() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        // Terminate when nothing is running should be ok
        let result = supervisor.terminate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_supervisor_take_stdin_stdout() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        // Spawn a process
        let result = supervisor.spawn("cat", &[], &HashMap::new(), None).await;
        assert!(result.is_ok());

        // Take stdin
        let stdin = supervisor.take_stdin();
        assert!(stdin.is_some());

        // Second take should return None
        let stdin2 = supervisor.take_stdin();
        assert!(stdin2.is_none());

        // Take stdout
        let stdout = supervisor.take_stdout();
        assert!(stdout.is_some());

        // Second take should return None
        let stdout2 = supervisor.take_stdout();
        assert!(stdout2.is_none());

        // Cleanup
        let _ = supervisor.terminate().await;
    }

    #[tokio::test]
    async fn test_supervisor_restart_increments_count() {
        let mut supervisor =
            ProcessSupervisor::new(5, Duration::from_millis(10), Duration::from_millis(100));

        // Initial spawn
        let result = supervisor.spawn("true", &[], &HashMap::new(), None).await;
        assert!(result.is_ok());
        assert_eq!(supervisor.restart_count(), 0);

        // Wait for exit
        tokio::time::sleep(Duration::from_millis(50)).await;
        supervisor.check_status();

        // Restart
        let restart_result = supervisor.restart("true", &[], &HashMap::new(), None).await;
        assert!(restart_result.is_ok());
        assert_eq!(supervisor.restart_count(), 1);
    }

    #[tokio::test]
    async fn test_supervisor_max_restarts_exceeded() {
        let mut supervisor =
            ProcessSupervisor::new(2, Duration::from_millis(10), Duration::from_millis(50));

        // Use up all restarts
        for i in 0..2 {
            let result = supervisor.restart("true", &[], &HashMap::new(), None).await;
            assert!(result.is_ok(), "Restart {} should succeed", i + 1);
            tokio::time::sleep(Duration::from_millis(20)).await;
            supervisor.check_status();
        }

        assert_eq!(supervisor.restart_count(), 2);

        // Next restart should fail
        let result = supervisor.restart("true", &[], &HashMap::new(), None).await;

        assert!(matches!(
            result,
            Err(SupervisorError::MaxRestartsExceeded(2))
        ));
        assert!(supervisor.is_permanently_failed());
    }

    #[tokio::test]
    async fn test_supervisor_restart_with_environment() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(10), Duration::from_millis(100));

        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());
        env.insert("ANOTHER_VAR".to_string(), "another_value".to_string());

        let result = supervisor.spawn("env", &[], &env, None).await;
        assert!(result.is_ok());

        // Process should have started
        assert!(matches!(supervisor.state(), ProcessState::Running { .. }));

        // Cleanup
        let _ = supervisor.terminate().await;
    }

    #[tokio::test]
    async fn test_supervisor_spawn_with_working_dir() {
        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        let temp_dir = std::env::temp_dir();

        let result = supervisor.spawn("pwd", &[], &HashMap::new(), Some(&temp_dir)).await;
        assert!(result.is_ok());

        // Process should have started
        assert!(supervisor.is_running());

        // Cleanup
        let _ = supervisor.terminate().await;
    }

    #[tokio::test]
    async fn test_supervisor_last_restart_updated() {
        let mut supervisor =
            ProcessSupervisor::new(5, Duration::from_millis(10), Duration::from_millis(50));

        assert!(supervisor.last_restart().is_none());

        // Do a restart
        let result = supervisor.restart("true", &[], &HashMap::new(), None).await;
        assert!(result.is_ok());

        assert!(supervisor.last_restart().is_some());

        // Verify the timestamp is recent
        let last_restart = supervisor.last_restart().unwrap();
        let elapsed = last_restart.elapsed();
        assert!(elapsed < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_supervisor_reset_backoff() {
        let mut supervisor =
            ProcessSupervisor::new(5, Duration::from_millis(100), Duration::from_secs(10));

        // Do some restarts to advance backoff
        for _ in 0..3 {
            let _ = supervisor.restart("true", &[], &HashMap::new(), None).await;
            tokio::time::sleep(Duration::from_millis(20)).await;
            supervisor.check_status();
        }

        assert_eq!(supervisor.backoff.current_attempt(), 3);

        // Reset backoff
        supervisor.reset_backoff();

        assert_eq!(supervisor.backoff.current_attempt(), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_supervisor_signal_detection() {
        use nix::sys::signal::{Signal, kill};
        use nix::unistd::Pid;

        let mut supervisor =
            ProcessSupervisor::new(3, Duration::from_millis(100), Duration::from_secs(1));

        // Spawn a long-running process
        let result = supervisor.spawn("sleep", &["60".to_string()], &HashMap::new(), None).await;
        assert!(result.is_ok());

        // Get the PID
        let pid = match supervisor.state() {
            ProcessState::Running { pid } => *pid,
            _ => panic!("Expected running state"),
        };

        // Send SIGKILL directly
        let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);

        // Wait for process to die
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check status should detect signal
        let state = supervisor.check_status();

        assert!(matches!(state, ProcessState::Signaled { signal: 9 }));
    }
}
