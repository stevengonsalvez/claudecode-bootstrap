#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "pytest>=7.0.0",
#     "python-dotenv",
# ]
# ///

# ABOUTME: Test suite for Claude Code hooks to ensure they trigger expected behaviors
# Tests the pre_compact handover trigger and session_start git status functionality

import json
import subprocess
import tempfile
import os
import pytest
from pathlib import Path
from unittest.mock import patch, MagicMock


class TestPreCompactHook:
    """Test pre_compact.py hook triggers /handover command."""
    
    def test_pre_compact_triggers_handover_command(self):
        """Test that pre_compact hook triggers /handover command when --handover flag is used."""
        # Arrange
        hook_path = Path(__file__).parent / "pre_compact.py"
        test_input = {
            "session_id": "test-session-123",
            "transcript_path": "/path/to/transcript.jsonl", 
            "trigger": "manual",
            "custom_instructions": "Test handover"
        }
        
        # Act
        result = subprocess.run([
            "uv", "run", str(hook_path), "--handover"
        ], 
        input=json.dumps(test_input),
        text=True,
        capture_output=True
        )
        
        # Assert - Should trigger handover command via JSON output
        assert result.returncode == 0
        assert "hookSpecificOutput" in result.stdout
        assert "handover" in result.stdout.lower()
    
    def test_pre_compact_without_handover_flag_works_normally(self):
        """Test that pre_compact hook works normally without --handover flag."""
        # Arrange
        hook_path = Path(__file__).parent / "pre_compact.py"
        test_input = {
            "session_id": "test-session-456",
            "transcript_path": "/path/to/transcript.jsonl",
            "trigger": "auto"
        }
        
        # Act
        result = subprocess.run([
            "uv", "run", str(hook_path), "--verbose"
        ],
        input=json.dumps(test_input),
        text=True, 
        capture_output=True
        )
        
        # Assert - Should work normally without handover
        assert result.returncode == 0
        assert "/handover" not in result.stdout


class TestSessionStartHook:
    """Test session_start.py hook runs git status."""
    
    def test_session_start_runs_git_status(self):
        """Test that session_start hook runs git status when --git-status flag is used."""
        # Arrange
        hook_path = Path(__file__).parent / "session_start.py"
        test_input = {
            "session_id": "session-789",
            "source": "startup"
        }
        
        # Act
        result = subprocess.run([
            "uv", "run", str(hook_path), "--git-status"
        ],
        input=json.dumps(test_input),
        text=True,
        capture_output=True
        )
        
        # Assert - Should run git status via JSON output
        assert result.returncode == 0
        # Should contain git status information in JSON format
        assert "hookSpecificOutput" in result.stdout
        output_lower = result.stdout.lower()
        assert any(word in output_lower for word in ["git", "status", "branch", "changes"])
    
    def test_session_start_without_git_flag_works_normally(self):
        """Test that session_start hook works normally without --git-status flag."""
        # Arrange  
        hook_path = Path(__file__).parent / "session_start.py"
        test_input = {
            "session_id": "session-101112", 
            "source": "resume"
        }
        
        # Act
        result = subprocess.run([
            "uv", "run", str(hook_path)
        ],
        input=json.dumps(test_input),
        text=True,
        capture_output=True
        )
        
        # Assert - Should work normally
        assert result.returncode == 0


class TestHooksIntegration:
    """Test hooks work together without conflicts."""
    
    def test_both_hooks_can_run_independently(self):
        """Test that both hooks can run without interfering with each other."""
        # Test pre_compact hook
        pre_compact_path = Path(__file__).parent / "pre_compact.py"
        pre_compact_input = {
            "session_id": "integration-test-1",
            "transcript_path": "/tmp/test.jsonl",
            "trigger": "manual"
        }
        
        pre_compact_result = subprocess.run([
            "uv", "run", str(pre_compact_path)
        ],
        input=json.dumps(pre_compact_input),
        text=True,
        capture_output=True
        )
        
        # Test session_start hook
        session_start_path = Path(__file__).parent / "session_start.py" 
        session_start_input = {
            "session_id": "integration-test-2",
            "source": "startup"
        }
        
        session_start_result = subprocess.run([
            "uv", "run", str(session_start_path)
        ],
        input=json.dumps(session_start_input),
        text=True,
        capture_output=True
        )
        
        # Assert both work independently
        assert pre_compact_result.returncode == 0
        assert session_start_result.returncode == 0


if __name__ == "__main__":
    # Run tests if executed directly
    import sys
    pytest.main([__file__] + sys.argv[1:])
