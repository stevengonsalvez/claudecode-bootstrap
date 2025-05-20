#!/usr/bin/env node

const inquirer = require('inquirer');
const fs = require('fs');
const path = require('path');

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

    // Step 3: List available rules for the tool
    const config = TOOL_CONFIG[tool];
    const rulePath = path.join(__dirname, config.ruleDir, config.ruleGlob);
    if (!fs.existsSync(rulePath)) {
        console.error(`No rule found for ${tool}`);
        process.exit(1);
    }

    // For now, only one rule per tool, but structure allows for more in future
    const rules = [config.ruleGlob];
    const { selectedRules } = await inquirer.prompt([
        {
            type: 'checkbox',
            name: 'selectedRules',
            message: 'Select rules to copy:',
            choices: rules,
            default: rules,
            validate: (arr) => arr.length > 0 || 'Select at least one rule',
        },
    ]);

    // Step 4: Copy selected rules
    for (const ruleFile of selectedRules) {
        const src = path.join(__dirname, config.ruleDir, ruleFile);
        const destDir = path.join(targetFolder, config.targetSubdir);
        const dest = path.join(destDir, ruleFile);
        fs.mkdirSync(destDir, { recursive: true });
        fs.copyFileSync(src, dest);
        console.log(`Copied ${ruleFile} to ${dest}`);
    }

    console.log('Done.');
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
}); 