#!/usr/bin/env node

import inquirer from 'inquirer';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { readdirSync, statSync } from 'fs';
import yaml from 'js-yaml';
import os from 'os';

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
    'claude-code': {
        ruleDir: 'claude-code',
        targetSubdir: '.claude',
        copyEntireFolder: true,
    },
    gemini: {
        ruleDir: 'gemini',
        targetSubdir: '.gemini',
        copyEntireFolder: true,
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

function parseFrontMatter(filePath) {
    const content = fs.readFileSync(filePath, 'utf8');
    const match = content.match(/^---([\s\S]*?)---/);
    if (!match) return {};
    try {
        return yaml.load(match[1]);
    } catch {
        return {};
    }
}

function showProgress(message, isComplete = false) {
    const greenCheck = '\x1b[32m✓\x1b[0m';
    
    if (isComplete) {
        console.log(`${greenCheck} ${message}`);
    } else {
        process.stdout.write(`⠋ ${message}...`);
    }
}

function completeProgress(message) {
    process.stdout.write('\r\x1b[K'); // Clear the line
    showProgress(message, true);
}

function copyDirectoryRecursive(source, destination) {
    const files = [];
    
    function getAllFiles(dir, basePath = '') {
        const items = readdirSync(dir);
        for (const item of items) {
            const fullPath = path.join(dir, item);
            const relativePath = path.join(basePath, item);
            
            if (statSync(fullPath).isDirectory()) {
                getAllFiles(fullPath, relativePath);
            } else {
                files.push({ source: fullPath, dest: path.join(destination, relativePath) });
            }
        }
    }
    
    getAllFiles(source);
    
    for (const file of files) {
        const destDir = path.dirname(file.dest);
        fs.mkdirSync(destDir, { recursive: true });
        fs.copyFileSync(file.source, file.dest);
    }
    
    return files.length;
}

async function handleFullDirectoryCopy(tool, config) {
    const homeDir = os.homedir();
    const destDir = path.join(homeDir, config.targetSubdir);

    showProgress(`Checking ~/${config.targetSubdir} directory`);
    if (!fs.existsSync(destDir)) {
        fs.mkdirSync(destDir, { recursive: true });
        completeProgress(`Created ~/${config.targetSubdir} directory`);
    } else {
        completeProgress(`Found ~/${config.targetSubdir} directory`);
    }

    showProgress(`Copying ${config.ruleDir} contents`);
    const sourceDir = path.join(__dirname, config.ruleDir);
    const filesCopied = copyDirectoryRecursive(sourceDir, destDir);
    completeProgress(`Copied ${filesCopied} files to ~/${config.targetSubdir}`);

    console.log(`\n\x1b[32m🎉 ${tool} setup complete!\x1b[0m`);
    console.log(`Files copied to: ${destDir}`);
}

async function main() {
    const args = process.argv.slice(2);
    const toolArg = args.find(arg => arg.startsWith('--tool='));
    const targetFolderArg = args.find(arg => arg.startsWith('--targetFolder='));
    const isNonInteractive = !!toolArg;

    let tool = toolArg ? toolArg.split('=')[1] : null;
    let targetFolder = targetFolderArg ? targetFolderArg.split('=')[1] : null;

    if (!tool) {
        const answers = await inquirer.prompt([
            {
                type: 'list',
                name: 'tool',
                message: 'Select the tool:',
                choices: Object.keys(TOOL_CONFIG),
            },
        ]);
        tool = answers.tool;
    }

    const config = TOOL_CONFIG[tool];

    if (config.copyEntireFolder) {
        await handleFullDirectoryCopy(tool, config);
        return;
    }

    if (!targetFolder) {
        const answers = await inquirer.prompt([
            {
                type: 'input',
                name: 'targetFolder',
                message: 'Enter the target project folder:',
                validate: (input) => !!input.trim() || 'Folder name required',
            },
        ]);
        targetFolder = answers.targetFolder;
    }
    
    showProgress('Creating target directory');
    if (!fs.existsSync(targetFolder)) {
        fs.mkdirSync(targetFolder, { recursive: true });
        completeProgress(`Created folder: ${targetFolder}`);
    } else {
        completeProgress(`Using existing folder: ${targetFolder}`);
    }

    showProgress('Copying tool-specific rules');
    const destDir = path.join(targetFolder, config.targetSubdir);
    fs.mkdirSync(destDir, { recursive: true });
    
    const rulePath = path.join(__dirname, config.ruleDir, config.ruleGlob);
    fs.copyFileSync(rulePath, path.join(destDir, config.ruleGlob));
    
    for (const rule of ALWAYS_COPY_RULES) {
        fs.copyFileSync(path.join(GENERAL_RULES_DIR, rule), path.join(destDir, rule));
    }
    completeProgress('Copied core rules');

    const copiedFiles = [config.ruleGlob, ...ALWAYS_COPY_RULES];
    const generalRuleFiles = getGeneralRuleFiles();
    if (!isNonInteractive && generalRuleFiles.length > 0) {
        const { selectedGeneralRules } = await inquirer.prompt([
            {
                type: 'checkbox',
                name: 'selectedGeneralRules',
                message: 'Select additional general rules to copy:',
                choices: generalRuleFiles,
            },
        ]);
        
        if (selectedGeneralRules && selectedGeneralRules.length > 0) {
            showProgress('Copying additional rules');
            for (const ruleFile of selectedGeneralRules) {
                fs.copyFileSync(path.join(GENERAL_RULES_DIR, ruleFile), path.join(destDir, ruleFile));
                copiedFiles.push(ruleFile);
            }
            completeProgress(`Copied ${selectedGeneralRules.length} additional rules`);
        }
    }

    showProgress('Generating rule registry');
    const registry = {};
    for (const file of copiedFiles) {
        const filePath = path.join(destDir, file);
        const front = parseFrontMatter(filePath);
        if (front) {
            registry[file.replace(/\..*$/, '')] = {
                path: path.join(config.targetSubdir, file),
                globs: Array.isArray(front.globs) ? front.globs : (front.globs ? [front.globs] : []),
                alwaysApply: !!front.alwaysApply
            };
        }
    }
    fs.writeFileSync(path.join(destDir, 'rule-registry.json'), JSON.stringify(registry, null, 4));
    completeProgress('Generated rule registry');

    console.log('\n\x1b[32m🎉 Setup complete!\x1b[0m');
    console.log(`Files copied to: ${destDir}`);
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
});