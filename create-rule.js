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
        excludeFiles: ['settings.local.json'],
        templateSubstitutions: {
            'CLAUDE.md': {
                'TOOL_DIR': '.claude',
                'HOME_TOOL_DIR': '~/.claude'
            }
        }
    },
    gemini: {
        ruleGlob: 'GEMINI.md',
        ruleDir: 'gemini',
        targetSubdir: '.gemini',
        sharedContentDir: 'claude-code',
        copySharedContent: true,
        excludeFiles: ['CLAUDE.md', 'settings.local.json'],
        settingsFile: 'gemini/settings.json',
        templateSubstitutions: {
            'GEMINI.md': {
                'TOOL_DIR': '.gemini',
                'HOME_TOOL_DIR': '.gemini'
            }
        }
    },
    amazonq: {
        ruleGlob: 'q-rulestore-rule.md',
        ruleDir: 'amazonq',
        targetSubdir: '.amazonq/rules',
        rootFiles: ['amazonq/AmazonQ.md'],
        mcpFile: 'amazonq/mcp.json',
        mcpTarget: '.amazonq/mcp.json',
        sharedContentDir: 'claude-code',
        copySharedContent: true,
        excludeFiles: ['CLAUDE.md', 'settings.local.json'],
        templateSubstitutions: {
            'AmazonQ.md': {
                'TOOL_DIR': '.amazonq',
                'HOME_TOOL_DIR': '.amazonq'
            }
        }
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

function substituteTemplate(content, substitutions) {
    let result = content;
    for (const [placeholder, value] of Object.entries(substitutions)) {
        const regex = new RegExp(`{{${placeholder}}}`, 'g');
        result = result.replace(regex, value);
    }
    return result;
}

function copyDirectoryRecursive(source, destination, excludeFiles = [], templateSubstitutions = {}) {
    const files = [];
    
    function getAllFiles(dir, basePath = '') {
        const items = readdirSync(dir);
        for (const item of items) {
            const fullPath = path.join(dir, item);
            const relativePath = path.join(basePath, item);
            
            if (statSync(fullPath).isDirectory()) {
                getAllFiles(fullPath, relativePath);
            } else {
                // Check if this file should be excluded
                const shouldExclude = excludeFiles.some(excludeFile => 
                    relativePath === excludeFile || item === excludeFile
                );
                
                if (!shouldExclude) {
                    files.push({ source: fullPath, dest: path.join(destination, relativePath), fileName: item });
                }
            }
        }
    }
    
    getAllFiles(source);
    
    for (const file of files) {
        const destDir = path.dirname(file.dest);
        fs.mkdirSync(destDir, { recursive: true });
        
        // Check if this file needs template substitution
        if (templateSubstitutions[file.fileName]) {
            let content = fs.readFileSync(file.source, 'utf8');
            content = substituteTemplate(content, templateSubstitutions[file.fileName]);
            fs.writeFileSync(file.dest, content);
        } else {
            fs.copyFileSync(file.source, file.dest);
        }
    }
    
    return files.length;
}

async function handleSharedContentCopy(tool, config, targetFolder) {
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

    const destDir = path.join(targetFolder, config.targetSubdir);
    fs.mkdirSync(destDir, { recursive: true });

    // Copy shared content from claude-code
    showProgress('Copying shared content from claude-code');
    const sharedSourceDir = path.join(__dirname, config.sharedContentDir);
    const sharedFilesCopied = copyDirectoryRecursive(sharedSourceDir, destDir, config.excludeFiles || [], config.templateSubstitutions || {});
    completeProgress(`Copied ${sharedFilesCopied} shared files`);

    // Copy tool-specific files
    showProgress('Copying tool-specific files');
    const toolSpecificPath = path.join(__dirname, config.ruleDir, config.ruleGlob);
    if (fs.existsSync(toolSpecificPath)) {
        // For GEMINI.md, copy CLAUDE.md and apply substitutions
        if (config.ruleGlob === 'GEMINI.md') {
            const claudePath = path.join(__dirname, 'claude-code', 'CLAUDE.md');
            let content = fs.readFileSync(claudePath, 'utf8');
            if (config.templateSubstitutions && config.templateSubstitutions['GEMINI.md']) {
                content = substituteTemplate(content, config.templateSubstitutions['GEMINI.md']);
            }
            fs.writeFileSync(path.join(destDir, config.ruleGlob), content);
        } else {
            fs.copyFileSync(toolSpecificPath, path.join(destDir, config.ruleGlob));
        }
        completeProgress('Copied tool-specific files');
    }

    // Copy always copy rules
    showProgress('Copying core rules');
    for (const rule of ALWAYS_COPY_RULES) {
        fs.copyFileSync(path.join(GENERAL_RULES_DIR, rule), path.join(destDir, rule));
    }
    completeProgress('Copied core rules');

    // Copy shared content to specific target if specified
    if (config.sharedContentTarget) {
        const targetDir = path.join(targetFolder, config.sharedContentTarget);
        showProgress(`Copying shared content to ${config.sharedContentTarget}`);
        const sharedSourceDir = path.join(__dirname, config.sharedContentDir);
        fs.mkdirSync(targetDir, { recursive: true });
        const sharedFilesCopied = copyDirectoryRecursive(sharedSourceDir, targetDir, config.excludeFiles || [], config.templateSubstitutions || {});
        completeProgress(`Copied ${sharedFilesCopied} shared files to ${config.sharedContentTarget}`);
    }

    // Copy settings file to project directory if specified
    if (config.settingsFile) {
        showProgress('Copying settings file');
        const sourcePath = path.join(__dirname, config.settingsFile);
        const destPath = path.join(destDir, 'settings.json');
        
        if (fs.existsSync(sourcePath)) {
            fs.copyFileSync(sourcePath, destPath);
            completeProgress('Copied settings file');
        }
    }

    // Copy MCP file to specified target if specified
    if (config.mcpFile && config.mcpTarget) {
        showProgress('Copying MCP configuration');
        const sourcePath = path.join(__dirname, config.mcpFile);
        const destPath = path.join(targetFolder, config.mcpTarget);
        
        if (fs.existsSync(sourcePath)) {
            fs.mkdirSync(path.dirname(destPath), { recursive: true });
            fs.copyFileSync(sourcePath, destPath);
            completeProgress(`Copied MCP config to ${config.mcpTarget}`);
        }
    }

    // Copy root files if they exist
    if (config.rootFiles) {
        showProgress('Copying root files');
        let rootFilesCopied = 0;
        for (const rootFile of config.rootFiles) {
            const sourcePath = path.join(__dirname, rootFile);
            const fileName = path.basename(rootFile);
            const destPath = path.join(targetFolder, fileName);
            
            // For AmazonQ.md, copy CLAUDE.md and apply substitutions
            if (fileName === 'AmazonQ.md') {
                const claudePath = path.join(__dirname, 'claude-code', 'CLAUDE.md');
                let content = fs.readFileSync(claudePath, 'utf8');
                if (config.templateSubstitutions && config.templateSubstitutions['AmazonQ.md']) {
                    content = substituteTemplate(content, config.templateSubstitutions['AmazonQ.md']);
                }
                fs.writeFileSync(destPath, content);
                rootFilesCopied++;
            } else if (fs.existsSync(sourcePath)) {
                fs.copyFileSync(sourcePath, destPath);
                rootFilesCopied++;
            }
        }
        completeProgress(`Copied ${rootFilesCopied} root files`);
    }

    console.log(`\n\x1b[32m🎉 ${tool} setup complete!\x1b[0m`);
    console.log(`Files copied to: ${destDir}`);
}

async function handleFullDirectoryCopy(tool, config, overrideHomeDir = null) {
    const homeDir = overrideHomeDir || os.homedir();
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
    const filesCopied = copyDirectoryRecursive(sourceDir, destDir, config.excludeFiles || [], config.templateSubstitutions || {});
    completeProgress(`Copied ${filesCopied} files to ~/${config.targetSubdir}`);

    // Copy tool-specific files if they exist
    if (config.toolSpecificFiles) {
        showProgress('Copying tool-specific files');
        let toolFilesCopied = 0;
        for (const toolFile of config.toolSpecificFiles) {
            const sourcePath = path.join(__dirname, toolFile);
            const fileName = path.basename(toolFile);
            const destPath = path.join(destDir, fileName);
            
            if (fs.existsSync(sourcePath)) {
                fs.copyFileSync(sourcePath, destPath);
                toolFilesCopied++;
            }
        }
        completeProgress(`Copied ${toolFilesCopied} tool-specific files`);
    }

    console.log(`\n\x1b[32m🎉 ${tool} setup complete!\x1b[0m`);
    console.log(`Files copied to: ${destDir}`);
}

async function main() {
    const args = process.argv.slice(2);
    const toolArg = args.find(arg => arg.startsWith('--tool='));
    const targetFolderArg = args.find(arg => arg.startsWith('--targetFolder='));
    const homeDirArg = args.find(arg => arg.startsWith('--homeDir='));
    const isNonInteractive = !!toolArg;

    let tool = toolArg ? toolArg.split('=')[1] : null;
    let targetFolder = targetFolderArg ? targetFolderArg.split('=')[1] : null;
    let overrideHomeDir = homeDirArg ? homeDirArg.split('=')[1] : null;

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
        await handleFullDirectoryCopy(tool, config, overrideHomeDir);
        return;
    }

    if (config.copySharedContent) {
        await handleSharedContentCopy(tool, config, targetFolder);
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

    // Copy shared content to specific target if specified
    if (config.sharedContentTarget) {
        const targetDir = path.join(targetFolder, config.sharedContentTarget);
        showProgress(`Copying shared content to ${config.sharedContentTarget}`);
        const sharedSourceDir = path.join(__dirname, config.sharedContentDir);
        fs.mkdirSync(targetDir, { recursive: true });
        const sharedFilesCopied = copyDirectoryRecursive(sharedSourceDir, targetDir, config.excludeFiles || [], config.templateSubstitutions || {});
        completeProgress(`Copied ${sharedFilesCopied} shared files to ${config.sharedContentTarget}`);
    }

    // Copy MCP file to specified target if specified
    if (config.mcpFile && config.mcpTarget) {
        showProgress('Copying MCP configuration');
        const sourcePath = path.join(__dirname, config.mcpFile);
        const destPath = path.join(targetFolder, config.mcpTarget);
        
        if (fs.existsSync(sourcePath)) {
            fs.mkdirSync(path.dirname(destPath), { recursive: true });
            fs.copyFileSync(sourcePath, destPath);
            completeProgress(`Copied MCP config to ${config.mcpTarget}`);
        }
    }

    // Copy root files if they exist
    if (config.rootFiles) {
        showProgress('Copying root files');
        let rootFilesCopied = 0;
        for (const rootFile of config.rootFiles) {
            const sourcePath = path.join(__dirname, rootFile);
            const fileName = path.basename(rootFile);
            const destPath = path.join(targetFolder, fileName);
            
            // For AmazonQ.md, copy CLAUDE.md and apply substitutions
            if (fileName === 'AmazonQ.md') {
                const claudePath = path.join(__dirname, 'claude-code', 'CLAUDE.md');
                let content = fs.readFileSync(claudePath, 'utf8');
                if (config.templateSubstitutions && config.templateSubstitutions['AmazonQ.md']) {
                    content = substituteTemplate(content, config.templateSubstitutions['AmazonQ.md']);
                }
                fs.writeFileSync(destPath, content);
                rootFilesCopied++;
            } else if (fs.existsSync(sourcePath)) {
                fs.copyFileSync(sourcePath, destPath);
                rootFilesCopied++;
            }
        }
        completeProgress(`Copied ${rootFilesCopied} root files`);
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