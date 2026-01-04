// ABOUTME: Workspace data model representing a git repository that can contain multiple sessions

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::Session;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub path: PathBuf,
    pub sessions: Vec<Session>,
}

impl Workspace {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            sessions: Vec::new(),
        }
    }

    pub fn add_session(&mut self, session: Session) {
        self.sessions.push(session);
    }

    pub fn remove_session(&mut self, session_id: &uuid::Uuid) -> bool {
        let initial_len = self.sessions.len();
        self.sessions.retain(|s| &s.id != session_id);
        self.sessions.len() != initial_len
    }

    pub fn get_session_mut(&mut self, session_id: &uuid::Uuid) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| &s.id == session_id)
    }

    pub fn get_session(&self, session_id: &uuid::Uuid) -> Option<&Session> {
        self.sessions.iter().find(|s| &s.id == session_id)
    }

    pub fn running_sessions(&self) -> Vec<&Session> {
        self.sessions.iter().filter(|s| s.status.is_running()).collect()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}
