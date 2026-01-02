#!/usr/bin/env python3
"""
ABOUTME: Statusline script for Claude Code showing project, git branch, changes, model, and context
Simple, reliable Python implementation that handles all input cases gracefully
"""

import sys
import json
import os
import subprocess
from pathlib import Path

# ANSI color codes
COLORS = {
    'cyan': '\033[36m',
    'green': '\033[32m',
    'magenta': '\033[35m',
    'gray': '\033[90m',
    'red': '\033[31m',
    'orange': '\033[38;5;208m',
    'yellow': '\033[33m',
    'reset': '\033[0m'
}

def run_git_command(cmd, cwd=None):
    """Run a git command and return output, empty string on error"""
    try:
        result = subprocess.run(
            cmd.split(),
            cwd=cwd or os.getcwd(),
            capture_output=True,
            text=True,
            timeout=0.5
        )
        return result.stdout.strip() if result.returncode == 0 else ''
    except:
        return ''

def get_context_tokens(transcript_path):
    """Calculate actual token usage from transcript"""
    if not transcript_path or not Path(transcript_path).exists():
        return 0
    
    try:
        # Read last few KB of file for performance
        with open(transcript_path, 'rb') as f:
            f.seek(0, 2)  # Go to end
            file_size = f.tell()
            read_size = min(8192, file_size)
            f.seek(max(0, file_size - read_size))
            data = f.read().decode('utf-8', errors='ignore')
        
        # Find the most recent usage info
        lines = data.strip().split('\n')
        for line in reversed(lines[-30:]):  # Check last 30 lines
            try:
                entry = json.loads(line)
                if 'message' in entry and 'usage' in entry['message']:
                    usage = entry['message']['usage']
                    if entry['message'].get('role') == 'assistant':
                        input_tokens = usage.get('input_tokens', 0)
                        output_tokens = usage.get('output_tokens', 0)
                        total_tokens = input_tokens + output_tokens
                        return total_tokens
            except:
                continue
    except:
        pass
    
    return 0

def get_model_short_name(model_info):
    """Extract short model name from model object or string"""
    # Handle both object format and string format for backward compatibility
    if isinstance(model_info, dict):
        model_name = model_info.get('display_name', model_info.get('id', 'Claude'))
    else:
        model_name = str(model_info)
    
    if 'opus' in model_name.lower():
        return 'Opus'
    elif 'sonnet' in model_name.lower():
        return 'Sonnet' 
    elif 'haiku' in model_name.lower():
        return 'Haiku'
    return 'Claude'

def format_token_count(tokens):
    """Format token count for display (e.g. 12500 -> '12.5k')"""
    if tokens == 0:
        return '0'
    elif tokens < 1000:
        return str(tokens)
    elif tokens < 1000000:
        return f"{tokens/1000:.1f}k"
    else:
        return f"{tokens/1000000:.1f}M"

def replace_home_with_tilde(path):
    """Replace home directory with ~ in path"""
    home = os.path.expanduser('~')
    if path.startswith(home):
        return path.replace(home, '~', 1)
    return path

def build_statusline():
    """Build the statusline string"""
    # Parse input - handle all cases
    input_data = {}
    try:
        raw_input = sys.stdin.read()
        if raw_input.strip():
            input_data = json.loads(raw_input)
    except:
        pass  # Use defaults
    
    # Extract values with defaults
    cwd = input_data.get('cwd', os.getcwd())
    model = input_data.get('model', 'claude-3-5-sonnet')
    session_id = input_data.get('session_id', 'default')
    transcript_path = input_data.get('transcript_path', '')
    
    # Get short model name (handles both object and string format)
    model_short = get_model_short_name(model)
    
    # Replace home directory with tilde in current directory
    display_cwd = replace_home_with_tilde(cwd)
    
    # Check if we're in a git repo
    git_root = run_git_command('git rev-parse --show-toplevel', cwd)
    
    if not git_root:
        # Not in git repo - simple status with current directory and model
        dir_name = Path(display_cwd).name
        tokens = get_context_tokens(transcript_path)
        token_display = format_token_count(tokens)
        
        components = []
        components.append(f"{COLORS['cyan']}{dir_name}{COLORS['reset']}")
        components.append(f"{COLORS['orange']}{model_short}{COLORS['reset']}")
        if tokens > 0:
            components.append(f"{COLORS['gray']}{token_display} tokens{COLORS['reset']}")
        
        return ' | '.join(components)
    
    # Get git information
    branch = run_git_command('git branch --show-current', cwd) or 'HEAD'
    
    # Count git changes
    status_output = run_git_command('git status --porcelain', cwd)
    changes_count = len(status_output.split('\n')) if status_output else 0
    
    # Build components
    components = []
    
    # Project directory
    project_name = Path(git_root).name
    components.append(f"{COLORS['cyan']}{project_name}{COLORS['reset']}")
    
    # Git branch with color
    if branch in ['main', 'master']:
        branch_color = COLORS['green']
    elif branch.startswith('feature/'):
        branch_color = COLORS['cyan']
    elif branch.startswith('fix/'):
        branch_color = COLORS['orange']
    elif branch.startswith('hotfix/'):
        branch_color = COLORS['red']
    else:
        branch_color = COLORS['magenta']
    
    components.append(f"{branch_color}{branch}{COLORS['reset']}")
    
    # Git changes
    if changes_count > 0:
        if changes_count > 10:
            change_color = COLORS['red']
        elif changes_count > 5:
            change_color = COLORS['yellow']
        else:
            change_color = COLORS['green']
        components.append(f"{change_color}+{changes_count}{COLORS['reset']}")
    
    # Model and token usage
    tokens = get_context_tokens(transcript_path)
    token_display = format_token_count(tokens)
    
    # Color based on token usage (rough thresholds)
    if tokens > 150000:
        token_color = COLORS['red']
    elif tokens > 100000:
        token_color = COLORS['yellow']
    else:
        token_color = COLORS['green']
    
    model_component = f"{COLORS['orange']}{model_short}{COLORS['reset']}"
    if tokens > 0:
        model_component += f" {token_color}{token_display} tokens{COLORS['reset']}"
    
    components.append(model_component)
    
    # Session summary (optional - if token usage > 10k)
    if tokens > 10000 and transcript_path and Path(transcript_path).exists():
        try:
            # Try to get first user message for summary
            with open(transcript_path, 'r') as f:
                for line in f:
                    try:
                        entry = json.loads(line)
                        if entry.get('message', {}).get('role') == 'user':
                            content = entry['message'].get('content', '')
                            if isinstance(content, list):
                                for item in content:
                                    if item.get('type') == 'text':
                                        content = item.get('text', '')
                                        break
                            if content:
                                summary = content[:50].replace('\n', ' ').strip()
                                if len(content) > 50:
                                    summary += '...'
                                components.append(f"{COLORS['gray']}# {summary}{COLORS['reset']}")
                                break
                    except:
                        continue
        except:
            pass
    
    return ' | '.join(components)

if __name__ == '__main__':
    try:
        statusline = build_statusline()
        sys.stdout.write(statusline)
        sys.stdout.flush()
    except Exception as e:
        # On any error, show a simple fallback
        sys.stdout.write(f"{COLORS['red']}status error{COLORS['reset']}")
        sys.stdout.flush()
