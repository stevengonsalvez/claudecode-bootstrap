#!/usr/bin/env bun

"use strict";

const fs = require("fs");
const { execSync } = require("child_process");
const path = require("path");

// ANSI color constants for terminal coloring
const c = {
    cy: '\033[36m',     // cyan
    g: '\033[32m',      // green
    m: '\033[35m',      // magenta
    gr: '\033[90m',     // gray
    r: '\033[31m',      // red
    o: '\033[38;5;208m', // orange
    y: '\033[33m',      // yellow
    x: '\033[0m'        // reset
};

// Cache for expensive operations
const cache = new Map();
const CACHE_TTL = 1000; // 1 second cache TTL for dynamic data

// Unified execution function with error handling and caching
const exec = (cmd, cwd = null, useCache = false) => {
    const cacheKey = `${cmd}:${cwd || 'default'}`;
    
    if (useCache && cache.has(cacheKey)) {
        const cached = cache.get(cacheKey);
        if (Date.now() - cached.timestamp < CACHE_TTL) {
            return cached.value;
        }
    }
    
    try {
        const options = { 
            encoding: 'utf8', 
            stdio: ['ignore', 'pipe', 'ignore'],
            timeout: 100 // 100ms timeout for git commands
        };
        if (cwd) options.cwd = cwd;
        const result = execSync(cmd, options).trim();
        
        if (useCache) {
            cache.set(cacheKey, { value: result, timestamp: Date.now() });
        }
        
        return result;
    } catch {
        return '';
    }
};

// Fast context percentage calculation - optimized for performance
function getContextPct(transcriptPath) {
    if (!transcriptPath || !fs.existsSync(transcriptPath)) return "0";
    
    try {
        // Use streaming for large files
        const stats = fs.statSync(transcriptPath);
        if (stats.size === 0) return "0";
        
        // Read only last 8KB of file for performance
        const bufferSize = Math.min(8192, stats.size);
        const buffer = Buffer.alloc(bufferSize);
        const fd = fs.openSync(transcriptPath, 'r');
        fs.readSync(fd, buffer, 0, bufferSize, Math.max(0, stats.size - bufferSize));
        fs.closeSync(fd);
        
        const data = buffer.toString('utf8');
        const lines = data.split('\n').filter(l => l.trim());
        
        let latestUsage = null;
        let latestTs = -Infinity;
        
        // Process only last 30 lines for speed
        for (let i = Math.max(0, lines.length - 30); i < lines.length; i++) {
            try {
                const j = JSON.parse(lines[i]);
                const ts = typeof j.timestamp === "string" ? 
                    new Date(j.timestamp).getTime() : j.timestamp;
                const usage = j.message?.usage;
                
                if (ts > latestTs && usage && j.message?.role === "assistant") {
                    latestTs = ts;
                    latestUsage = usage;
                }
            } catch {}
        }
        
        if (latestUsage) {
            const used = (latestUsage.input_tokens || 0) + (latestUsage.output_tokens || 0);
            const limit = latestUsage.cache_creation_input_tokens || 200000;
            const pct = Math.min(100, Math.round((used / limit) * 100));
            return pct.toString();
        }
    } catch {}
    
    return "0";
}

// Extract first user message from transcript
function getFirstUserMessage(transcriptPath) {
    if (!transcriptPath || !fs.existsSync(transcriptPath)) return null;
    
    try {
        const data = fs.readFileSync(transcriptPath, 'utf8');
        const lines = data.split('\n').slice(0, 50); // Check only first 50 lines
        
        for (const line of lines) {
            if (!line.trim()) continue;
            try {
                const j = JSON.parse(line);
                if (j.message?.role === "user" && j.message?.content) {
                    let content = j.message.content;
                    if (Array.isArray(content)) {
                        content = content.find(c => c.type === "text")?.text || "";
                    }
                    return content.slice(0, 100).replace(/\n/g, ' ').trim();
                }
            } catch {}
        }
    } catch {}
    
    return null;
}

// Get or generate session summary with caching
function getSessionSummary(transcriptPath, sessionId, gitDir, workingDir) {
    const summaryDir = path.join(gitDir, '.claude', 'summaries');
    const summaryFile = path.join(summaryDir, `${sessionId}.txt`);
    
    // Check if summary exists
    if (fs.existsSync(summaryFile)) {
        try {
            return fs.readFileSync(summaryFile, 'utf8').trim();
        } catch {}
    }
    
    // Generate new summary from first user message
    const firstMsg = getFirstUserMessage(transcriptPath);
    if (!firstMsg) return null;
    
    // Create summary using git branch context
    const branch = exec('git branch --show-current', workingDir, true) || 'main';
    let summary = firstMsg;
    
    if (branch && branch !== 'main' && branch !== 'master') {
        summary = `[${branch}] ${firstMsg}`;
    }
    
    // Save summary for future use
    try {
        if (!fs.existsSync(summaryDir)) {
            fs.mkdirSync(summaryDir, { recursive: true });
        }
        fs.writeFileSync(summaryFile, summary);
    } catch {}
    
    return summary;
}

// Cached PR lookup with fast git operations
function getPR(branch, workingDir) {
    if (!branch || branch === 'main' || branch === 'master') return '';
    
    const cacheKey = `pr:${branch}`;
    if (cache.has(cacheKey)) {
        const cached = cache.get(cacheKey);
        if (Date.now() - cached.timestamp < 5000) { // 5s cache for PR info
            return cached.value;
        }
    }
    
    // Try to get PR from push output or gh cli
    const prUrl = exec(`git config branch.${branch}.pr-url`, workingDir, true);
    if (prUrl) {
        const prNumber = prUrl.match(/\/pull\/(\d+)/)?.[1];
        if (prNumber) {
            const result = ` ${c.cy}PR#${prNumber}${c.x}`;
            cache.set(cacheKey, { value: result, timestamp: Date.now() });
            return result;
        }
    }
    
    return '';
}

// Main statusline function - optimized for speed
function statusline() {
    try {
        // Parse input JSON
        const input = JSON.parse(fs.readFileSync(0, 'utf8'));
        const workingDir = input.cwd || process.cwd();
        const model = input.model || 'claude-3-5-sonnet';
        const sessionId = input.session_id || 'default';
        const transcriptPath = input.transcript_path;
        
        // Quick model name extraction
        const modelShort = model.includes('opus') ? 'Opus' :
                          model.includes('sonnet') ? 'Sonnet' :
                          model.includes('haiku') ? 'Haiku' : 'Claude';
        
        // Fast git checks
        const gitDir = exec('git rev-parse --show-toplevel', workingDir, true);
        if (!gitDir) {
            // Not in git repo - simple status
            const dir = path.basename(workingDir);
            return `${c.cy}${dir}${c.x} | ${c.m}${modelShort}${c.x}`;
        }
        
        // Get git info with parallel execution where possible
        const branch = exec('git branch --show-current', workingDir, true) || 'HEAD';
        const isWorktree = exec('git rev-parse --is-inside-work-tree', workingDir) === 'true' &&
                          exec('git rev-parse --show-superproject-working-tree', workingDir) === '';
        
        // Fast git status check
        const statusOutput = exec('git status --porcelain', workingDir, true);
        const changes = statusOutput ? statusOutput.split('\n').length : 0;
        
        // Build status components
        const components = [];
        
        // Directory/worktree
        const dirName = path.basename(gitDir);
        if (isWorktree) {
            const worktreeName = path.basename(workingDir);
            components.push(`${c.gr}${dirName}${c.x}/${c.cy}${worktreeName}${c.x}`);
        } else {
            components.push(`${c.cy}${dirName}${c.x}`);
        }
        
        // Branch with color based on name
        const branchColor = branch === 'main' || branch === 'master' ? c.g :
                           branch.startsWith('feature/') ? c.cy :
                           branch.startsWith('fix/') ? c.o :
                           branch.startsWith('hotfix/') ? c.r : c.m;
        components.push(`${branchColor}${branch}${c.x}`);
        
        // Git changes
        if (changes > 0) {
            const changeColor = changes > 10 ? c.r : changes > 5 ? c.y : c.g;
            components.push(`${changeColor}+${changes}${c.x}`);
        }
        
        // PR info
        const pr = getPR(branch, workingDir);
        if (pr) components.push(pr);
        
        // Model and context
        const contextPct = getContextPct(transcriptPath);
        const contextNum = parseInt(contextPct);
        const contextColor = contextNum > 80 ? c.r : contextNum > 60 ? c.y : c.g;
        components.push(`${c.m}${modelShort}${c.x} ${contextColor}${contextPct}%${c.x}`);
        
        // Session summary (only if significant)
        if (contextNum > 10) {
            const summary = getSessionSummary(transcriptPath, sessionId, gitDir, workingDir);
            if (summary && summary.length > 10) {
                const truncated = summary.length > 50 ? 
                    summary.slice(0, 47) + '...' : summary;
                components.push(`${c.gr}# ${truncated}${c.x}`);
            }
        }
        
        return components.join(' | ');
        
    } catch (err) {
        // Fallback status on error
        return `${c.r}statusline error${c.x}`;
    }
}

// Output result immediately
process.stdout.write(statusline());
