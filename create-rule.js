#!/usr/bin/env node

import inquirer from 'inquirer';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { readdirSync, statSync } from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const TOOL_CONFIG = {
    amazonq: {
        ruleGlob: 'q-rulestore-rule.md',
        ruleDir: 'amazonq',
        targetSubdir: '.amazonq/rules',
    },
    cline: {
        ruleGlob: 'cline-rulestore-rule.md',
        ruleDir: 'cline',
        targetSubdir: '.clinerules',
    },
    roo: {
        ruleGlob: 'roo-rulestore-rule.md',
        ruleDir: 'roo',
        targetSubdir: '.roo/rules',
    },
    cursor: {
        ruleGlob: 'cursor-rulestore-rule.md',
        ruleDir: 'cursor',
        targetSubdir: '.cursor/rules',
    },
    claude: {
        ruleGlob: 'cursor-rulestore-rule.md',
        ruleDir: 'cursor',
        targetSubdir: '.claude/rules',
    },
};

const GENERAL_RULES_DIR = path.join(__dirname, 'general-rules');
const ALWAYS_COPY_RULES = [
    'rule-interpreter-rule.md',
    'rulestyle-rule.md',
];

function getGeneralRuleFiles() {
    return readdirSync(GENERAL_RULES_DIR)
        .filter(f => f.endsWith('.md'))
        .filter(f => !ALWAYS_COPY_RULES.includes(f));
}

async function main() {
    // Step 1: Select tool
    const { tool } = await inquirer.prompt([
        {
            type: 'list',
            name: 'tool',
            message: 'Select the tool:',
            choices: Object.keys(TOOL_CONFIG),
        },
    ]);

    // Step 2: Prompt for target project folder
    const { targetFolder } = await inquirer.prompt([
        {
            type: 'input',
            name: 'targetFolder',
            message: 'Enter the target project folder:',
            validate: (input) => !!input.trim() || 'Folder name required',
        },
    ]);
    if (!fs.existsSync(targetFolder)) {
        fs.mkdirSync(targetFolder, { recursive: true });
        console.log(`Created folder: ${targetFolder}`);
    }

    // Step 3: Copy always-included rules
    const config = TOOL_CONFIG[tool];
    const destDir = path.join(targetFolder, config.targetSubdir);
    fs.mkdirSync(destDir, { recursive: true });
    // Tool's own rulestore rule
    const rulePath = path.join(__dirname, config.ruleDir, config.ruleGlob);
    fs.copyFileSync(rulePath, path.join(destDir, config.ruleGlob));
    // Always-copy general rules
    for (const rule of ALWAYS_COPY_RULES) {
        fs.copyFileSync(path.join(GENERAL_RULES_DIR, rule), path.join(destDir, rule));
    }

    // Step 4: Prompt for additional general rules
    const generalRuleFiles = getGeneralRuleFiles();
    if (generalRuleFiles.length > 0) {
        const { selectedGeneralRules } = await inquirer.prompt([
            {
                type: 'checkbox',
                name: 'selectedGeneralRules',
                message: 'Select additional general rules to copy:',
                choices: generalRuleFiles,
            },
        ]);
        for (const ruleFile of selectedGeneralRules) {
            fs.copyFileSync(path.join(GENERAL_RULES_DIR, ruleFile), path.join(destDir, ruleFile));
        }
    }

    console.log('Done.');
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
}); 