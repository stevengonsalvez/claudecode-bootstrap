#!/usr/bin/env bun

"use strict";

const fs = require("fs");
const { execSync } = require("child_process");

// Read input from stdin
const input = fs.readFileSync(0, 'utf8');

try {
    // Parse the input to check if we need to process it for action-summary
    let inputData;
    try {
        inputData = JSON.parse(input);
    } catch {
        // If not JSON, pass through to ccstatusline as-is
        inputData = null;
    }
    
    // Run ccstatusline first
    let ccstatusOutput = '';
    try {
        ccstatusOutput = execSync('bunx -y ccstatusline@latest', {
            input: input,
            encoding: 'utf8',
            stdio: ['pipe', 'pipe', 'ignore'],
            timeout: 5000
        }).trim();
    } catch (err) {
        // If ccstatusline fails or times out, fallback to a simple status
        if (inputData) {
            const model = inputData.model || 'claude';
            const cwd = inputData.cwd || process.cwd();
            const dir = cwd.split('/').pop();
            ccstatusOutput = `${dir} | ${model}`;
        } else {
            ccstatusOutput = 'claude-code';
        }
    }
    
    // Run action-summary only if we have valid input data
    let actionSummaryOutput = '';
    // if (inputData && inputData.transcript_path) {
    //     try {
    //         actionSummaryOutput = execSync('bun ~/.claude/hooks/action-summary.js', {
    //             input: JSON.stringify(inputData),
    //             encoding: 'utf8',
    //             stdio: ['pipe', 'pipe', 'ignore'],
    //             timeout: 1000
    //         }).trim();
    //     } catch {
    //         // Silent fail for action summary
    //     }
    // }
    
    // Combine outputs
    if (actionSummaryOutput) {
        process.stdout.write(`${ccstatusOutput}\n${actionSummaryOutput}`);
    } else {
        process.stdout.write(ccstatusOutput);
    }
    
} catch (err) {
    // Fallback to simple output
    process.stdout.write('claude-code');
}
