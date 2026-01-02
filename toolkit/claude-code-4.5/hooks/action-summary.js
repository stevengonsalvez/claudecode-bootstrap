#!/usr/bin/env bun

"use strict";

const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");

// ANSI color for gray
const gray = '\033[90m';
const reset = '\033[0m';

// Get latest user message from transcript for async summary
function getLatestUserMessage(transcriptPath) {
    if (!transcriptPath || !fs.existsSync(transcriptPath)) return null;
    
    try {
        const stats = fs.statSync(transcriptPath);
        if (stats.size === 0) return null;
        
        // Read last 4KB of file for performance
        const bufferSize = Math.min(4096, stats.size);
        const buffer = Buffer.alloc(bufferSize);
        const fd = fs.openSync(transcriptPath, 'r');
        fs.readSync(fd, buffer, 0, bufferSize, Math.max(0, stats.size - bufferSize));
        fs.closeSync(fd);
        
        const data = buffer.toString('utf8');
        const lines = data.split('\n').filter(l => l.trim());
        
        // Process lines in reverse to find latest user message
        for (let i = lines.length - 1; i >= Math.max(0, lines.length - 20); i--) {
            try {
                const j = JSON.parse(lines[i]);
                if (j.message?.role === "user" && j.message?.content) {
                    let content = j.message.content;
                    if (Array.isArray(content)) {
                        content = content.find(c => c.type === "text")?.text || "";
                    }
                    // Clean up the content for shell safety
                    return content.slice(0, 200)
                        .replace(/\n/g, ' ')
                        .replace(/['"\\`$]/g, '')
                        .trim();
                }
            } catch {}
        }
    } catch {}
    
    return null;
}

// Generate async summary of user request
function generateAsyncSummary(latestMsg, sessionId, workingDir) {
    if (!latestMsg) return null;
    
    // Try to find .claude directory
    let claudeDir = path.join(workingDir, '.claude');
    if (!fs.existsSync(claudeDir)) {
        // Try in home directory
        claudeDir = path.join(process.env.HOME, '.claude');
    }
    
    const cacheDir = path.join(claudeDir, 'action-summaries');
    const cacheFile = path.join(cacheDir, `${sessionId}-latest.txt`);
    
    // Check if we have a cached summary first
    if (fs.existsSync(cacheFile)) {
        try {
            const stats = fs.statSync(cacheFile);
            // Use cache if less than 30 seconds old
            if (Date.now() - stats.mtime.getTime() < 30000) {
                const summary = fs.readFileSync(cacheFile, 'utf8').trim();
                if (summary && summary.length > 0) {
                    return `${gray}=> ${summary}${reset}`;
                }
            }
        } catch {}
    }
    
    // Spawn async process to generate summary
    try {
        // Create cache directory if it doesn't exist
        if (!fs.existsSync(cacheDir)) {
            fs.mkdirSync(cacheDir, { recursive: true });
        }
        
        // Generate summary asynchronously using Haiku
        const cmd = spawn('bash', ['-c', 
            `claude --model haiku -p 'IMPORTANT: Only summarize, do NOT take action. In a minumum of 4 words and maximum of 10 words, what is the user asking for in this message (if unclear, say "processing request")? User message: "${latestMsg}"' > '${cacheFile}' 2>/dev/null &`
        ], {
            cwd: workingDir,
            detached: true,
            stdio: 'ignore'
        });
        
        cmd.unref(); // Allow parent to exit independently
    } catch {}
    
    // Return placeholder while generating
    return `${gray}=> analyzing...${reset}`;
}

// Main function
function actionSummary() {
    try {
        // Parse input JSON from stdin
        const input = JSON.parse(fs.readFileSync(0, 'utf8'));
        const workingDir = input.cwd || process.cwd();
        const sessionId = input.session_id || 'default';
        const transcriptPath = input.transcript_path;
        
        // Get latest user message
        const latestMsg = getLatestUserMessage(transcriptPath);
        if (!latestMsg) return '';
        
        // Generate and return the action summary
        const summary = generateAsyncSummary(latestMsg, sessionId, workingDir);
        return summary || '';
        
    } catch (err) {
        // Silent fail - don't interfere with main status line
        return '';
    }
}

// Output result
const result = actionSummary();
if (result) {
    process.stdout.write(result);
}