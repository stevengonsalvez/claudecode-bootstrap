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
        excludeFiles: ['settings.local.json', 'skills'],
        templateSubstitutions: {
            'CLAUDE.md': {
                'TOOL_DIR': '.claude',
                'HOME_TOOL_DIR': '~/.claude'
            },
            '**/*.md': {
                'TOOL_DIR': '.claude',
                'HOME_TOOL_DIR': '~/.claude'
            }
        }
    },
    'claude-code-4.5': {
        ruleDir: 'claude-code-4.5',
        targetSubdir: '.claude',
        copyEntireFolder: true,
        excludeFiles: ['settings.local.json', 'skills'],
        templateSubstitutions: {
            'CLAUDE.md': {
                'TOOL_DIR': '.claude',
                'HOME_TOOL_DIR': '~/.claude'
            },
            '**/*.md': {
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
        mcpFile: 'amazonq/mcp.json',
        mcpTarget: '.amazonq/mcp.json',
        sharedContentDir: 'claude-code',
        specialCopies: [
            {
                source: 'amazonq/AmazonQ.md',
                dest: '.amazonq/rules/AmazonQ.md'
            }
        ],
        linkedFiles: [
            {
                source: 'amazonq/AmazonQ.md',
                linkName: 'AmazonQ.md'
            }
        ],
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

function getEffectiveExcludeFiles(tool, config) {
    const excludeFiles = [...(config.excludeFiles || [])];
    
    // Exclude agents folder for all tools except claude-code
    if (tool !== 'claude-code') {
        excludeFiles.push('agents');
    }
    
    return excludeFiles;
}

function copyDirectoryRecursive(source, destination, excludeFiles = [], templateSubstitutions = {}) {
    const files = [];

    function shouldSkipItem(item, relativePath, isDirectory) {
        // Skip node_modules and logs directories
        if (isDirectory && (item === 'node_modules' || item === 'logs')) {
            return true;
        }

        // Skip build artifacts and lock files
        if (!isDirectory) {
            // Skip Bun build temp files
            if (item.startsWith('.') && item.includes('.bun-build')) {
                return true;
            }
            // Skip lock files in bin directories
            if (relativePath.includes('/bin/') && (item === 'bun.lockb' || item === 'bun.lock')) {
                return true;
            }
        }

        // Check explicit excludes
        return excludeFiles.some(excludeFile =>
            relativePath === excludeFile || item === excludeFile
        );
    }

    function getAllFiles(dir, basePath = '') {
        const items = readdirSync(dir);
        for (const item of items) {
            const fullPath = path.join(dir, item);
            const relativePath = path.join(basePath, item);
            const isDirectory = statSync(fullPath).isDirectory();

            if (shouldSkipItem(item, relativePath, isDirectory)) {
                continue;
            }

            if (isDirectory) {
                getAllFiles(fullPath, relativePath);
            } else {
                files.push({ source: fullPath, dest: path.join(destination, relativePath), fileName: item, relativePath: relativePath });
            }
        }
    }
    
    getAllFiles(source);
    
    for (const file of files) {
        const destDir = path.dirname(file.dest);
        fs.mkdirSync(destDir, { recursive: true });
        
        // Check if this file needs template substitution
        // Support both exact filename and relative path matching
        const substitutions = templateSubstitutions[file.fileName] || 
                            templateSubstitutions[file.relativePath] || 
                            (file.relativePath.endsWith('.md') ? templateSubstitutions['**/*.md'] : null);
        
        if (substitutions) {
            let content = fs.readFileSync(file.source, 'utf8');
            content = substituteTemplate(content, substitutions);
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
    
    const excludeFiles = getEffectiveExcludeFiles(tool, config);
    const sharedFilesCopied = copyDirectoryRecursive(sharedSourceDir, destDir, excludeFiles, config.templateSubstitutions || {});
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
        const excludeFiles = getEffectiveExcludeFiles(tool, config);
        const sharedFilesCopied = copyDirectoryRecursive(sharedSourceDir, targetDir, excludeFiles, config.templateSubstitutions || {});
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

    // Perform special file copies
    if (config.specialCopies) {
        showProgress('Performing special file copies');
        let specialFilesCopied = 0;
        for (const copy of config.specialCopies) {
            const sourcePath = path.join(__dirname, copy.source);
            const destPath = path.join(targetFolder, copy.dest);
            const fileName = path.basename(copy.source);

            if (fs.existsSync(sourcePath)) {
                fs.mkdirSync(path.dirname(destPath), { recursive: true });

                if (config.templateSubstitutions && config.templateSubstitutions[fileName]) {
                    let content = fs.readFileSync(sourcePath, 'utf8');
                    content = substituteTemplate(content, config.templateSubstitutions[fileName]);
                    fs.writeFileSync(destPath, content);
                } else {
                    fs.copyFileSync(sourcePath, destPath);
                }
                specialFilesCopied++;
            }
        }
        completeProgress(`Copied ${specialFilesCopied} special files`);
    }

    

    // Create linked files if they exist
    if (config.linkedFiles) {
        showProgress('Creating linked files');
        let linkedFilesCreated = 0;
        for (const link of config.linkedFiles) {
            const linkPath = path.join(targetFolder, link.linkName);
            const sourcePath = path.join(config.targetSubdir, link.source.split('/').pop());
            fs.writeFileSync(linkPath, `@${sourcePath}`);
            linkedFilesCreated++;
        }
        completeProgress(`Created ${linkedFilesCreated} linked files`);
    }

    console.log(`\n\x1b[32m🎉 ${tool} setup complete!\x1b[0m`);
    console.log(`Files copied to: ${destDir}`);
}

async function handleFullDirectoryCopy(tool, config, overrideHomeDir = null, targetFolder = null) {
    let destDir;
    let displayPath;
    
    if (targetFolder) {
        // If targetFolder is specified, use it instead of home directory
        destDir = path.join(targetFolder, config.targetSubdir);
        displayPath = path.join(targetFolder, config.targetSubdir);
        showProgress(`Checking ${displayPath} directory`);
    } else {
        // Default behavior - use home directory
        const homeDir = overrideHomeDir || os.homedir();
        destDir = path.join(homeDir, config.targetSubdir);
        displayPath = `~/${config.targetSubdir}`;
        showProgress(`Checking ${displayPath} directory`);
    }
    
    if (!fs.existsSync(destDir)) {
        fs.mkdirSync(destDir, { recursive: true });
        completeProgress(`Created ${displayPath} directory`);
    } else {
        completeProgress(`Found ${displayPath} directory`);
    }

    showProgress(`Copying ${config.ruleDir} contents`);
    const sourceDir = path.join(__dirname, config.ruleDir);
    const filesCopied = copyDirectoryRecursive(sourceDir, destDir, config.excludeFiles || [], config.templateSubstitutions || {});
    completeProgress(`Copied ${filesCopied} files to ${displayPath}`);

    // Copy output-styles folder for claude-code
    if (tool === 'claude-code') {
        showProgress('Copying output-styles folder');
        const outputStylesSource = path.join(__dirname, 'claude-code', 'output-styles');
        const outputStylesDest = path.join(destDir, 'output-styles');

        if (fs.existsSync(outputStylesSource)) {
            const outputStylesFiles = copyDirectoryRecursive(outputStylesSource, outputStylesDest, [], {});
            completeProgress(`Copied ${outputStylesFiles} output-styles files`);
        }
    }

    // Copy skills folder for claude-code and claude-code-4.5
    if (tool === 'claude-code' || tool === 'claude-code-4.5') {
        showProgress('Copying skills folder');
        const skillsSource = path.join(__dirname, config.ruleDir, 'skills');
        const skillsDest = path.join(destDir, 'skills');

        if (fs.existsSync(skillsSource)) {
            const skillsFiles = copyDirectoryRecursive(skillsSource, skillsDest, [], config.templateSubstitutions || {});
            completeProgress(`Copied ${skillsFiles} skill files`);
        }

        // Smart compilation for browser-tools in webapp-testing skill
        const browserToolsDir = path.join(skillsDest, 'webapp-testing', 'bin');
        const browserToolsTs = path.join(browserToolsDir, 'browser-tools.ts');
        const browserToolsBinary = path.join(browserToolsDir, 'browser-tools');
        const rebuildFlag = process.argv.includes('--rebuild');

        if (fs.existsSync(browserToolsTs)) {
            // For claude-code-4.5: compile the binary
            if (tool === 'claude-code-4.5') {
                let shouldCompile = rebuildFlag;

                if (!fs.existsSync(browserToolsBinary)) {
                    shouldCompile = true;
                    showProgress('browser-tools binary missing, compiling');
                } else if (!rebuildFlag) {
                    const tsStats = statSync(browserToolsTs);
                    const binaryStats = statSync(browserToolsBinary);
                    if (tsStats.mtime > binaryStats.mtime) {
                        shouldCompile = true;
                        showProgress('browser-tools.ts is newer, recompiling');
                    }
                }

                if (shouldCompile) {
                    try {
                        showProgress('Compiling browser-tools binary');
                        const { execSync } = await import('child_process');

                        // Try Bun first
                        try {
                            execSync('which bun', { stdio: 'pipe' });
                            execSync(
                                `cd "${browserToolsDir}" && bun install && bun build browser-tools.ts --compile --target bun --outfile browser-tools`,
                                { stdio: 'pipe' }
                            );
                            completeProgress('Compiled browser-tools with Bun');
                        } catch {
                            // Fallback to esbuild
                            try {
                                execSync('which esbuild', { stdio: 'pipe' });
                                execSync(
                                    `cd "${browserToolsDir}" && npm install && esbuild browser-tools.ts --bundle --platform=node --outfile=browser-tools.js`,
                                    { stdio: 'pipe' }
                                );
                                completeProgress('Compiled browser-tools with esbuild (JavaScript output)');
                            } catch {
                                completeProgress('Compilation failed, using existing binary if available');
                            }
                        }
                    } catch (error) {
                        completeProgress(`Compilation error: ${error.message}, using existing binary if available`);
                    }
                }
            }
            // For claude-code: copy binary from claude-code-4.5 source
            else if (tool === 'claude-code') {
                showProgress('Copying browser-tools binary from claude-code-4.5');
                const sourceBinary = path.join(__dirname, 'claude-code-4.5', 'skills', 'webapp-testing', 'bin', 'browser-tools');

                // Remove symlink if it exists (from git source)
                if (fs.existsSync(browserToolsBinary)) {
                    fs.unlinkSync(browserToolsBinary);
                }

                // Copy binary from claude-code-4.5 source
                if (fs.existsSync(sourceBinary)) {
                    fs.copyFileSync(sourceBinary, browserToolsBinary);
                    fs.chmodSync(browserToolsBinary, 0o755); // Make executable
                    completeProgress('Copied browser-tools binary from claude-code-4.5');
                } else {
                    completeProgress('Warning: browser-tools binary not found in claude-code-4.5');
                }
            }
        }
    }

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
    const sddShortcut = args.includes('--sdd');
    const isNonInteractive = !!toolArg || sddShortcut;

    let tool = toolArg ? toolArg.split('=')[1] : null;
    let targetFolder = targetFolderArg ? targetFolderArg.split('=')[1] : null;
    let overrideHomeDir = homeDirArg ? homeDirArg.split('=')[1] : null;

    // Convenience: allow --sdd without specifying a tool
    if (!tool && sddShortcut) {
        tool = 'sdd';
    }

    if (!tool) {
        const answers = await inquirer.prompt([
            {
                type: 'list',
                name: 'tool',
                message: 'Select the tool:',
                choices: [...Object.keys(TOOL_CONFIG), 'sdd'],
            },
        ]);
        tool = answers.tool;
    }

    // Handle SDD-only installation mode (no rules), copying assets from spec-kit into a project
    if (tool === 'sdd') {
        // Single, simple path: clone repo then copy assets
        const repo = process.env.SPEC_KIT_REPO || 'https://github.com/github/spec-kit.git';
        const ref = process.env.SPEC_KIT_REF || '';
        // pick destination
        const dest = targetFolder || (await inquirer.prompt([{ type: 'input', name: 'sddDest', message: 'Project folder for Spec Kit (SDD) assets:', validate: (v) => !!v.trim() || 'Folder name required' }])).sddDest;
        const clonePath = await cloneSpecKit(repo, ref);
        await copySpecKitAssets(clonePath, dest);
        return;
    }

    const config = TOOL_CONFIG[tool];

    if (config.copyEntireFolder) {
        await handleFullDirectoryCopy(tool, config, overrideHomeDir, targetFolder);
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
        const excludeFiles = getEffectiveExcludeFiles(tool, config);
        const sharedFilesCopied = copyDirectoryRecursive(sharedSourceDir, targetDir, excludeFiles, config.templateSubstitutions || {});
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

    // Perform special file copies
    if (config.specialCopies) {
        showProgress('Performing special file copies');
        let specialFilesCopied = 0;
        for (const copy of config.specialCopies) {
            const sourcePath = path.join(__dirname, copy.source);
            const destPath = path.join(targetFolder, copy.dest);
            const fileName = path.basename(copy.source);

            if (fs.existsSync(sourcePath)) {
                fs.mkdirSync(path.dirname(destPath), { recursive: true });

                if (config.templateSubstitutions && config.templateSubstitutions[fileName]) {
                    let content = fs.readFileSync(sourcePath, 'utf8');
                    content = substituteTemplate(content, config.templateSubstitutions[fileName]);
                    fs.writeFileSync(destPath, content);
                } else {
                    fs.copyFileSync(sourcePath, destPath);
                }
                specialFilesCopied++;
            }
        }
        completeProgress(`Copied ${specialFilesCopied} special files`);
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

// --- Spec Kit integration: clone-on-demand, then copy ---
import { execSync } from 'child_process';

async function cloneSpecKit(repoUrl, ref = '') {
    const base = fs.mkdtempSync(path.join(os.tmpdir(), 'spec-kit-'));
    const cloneDir = path.join(base, 'src');
    showProgress(`Cloning Spec Kit from ${repoUrl}`);
    execSync(`git clone --depth 1 ${repoUrl} ${cloneDir}`, { stdio: 'pipe' });
    if (ref && ref.trim()) {
        execSync(`git -C ${cloneDir} fetch origin ${ref} --depth 1`, { stdio: 'pipe' });
        execSync(`git -C ${cloneDir} checkout ${ref}`, { stdio: 'pipe' });
    }
    completeProgress('Cloned Spec Kit');
    return cloneDir;
}
async function copySpecKitAssets(specKitRoot, projectRoot) {
    const requiredFiles = {
        claudeCommands: [
            { source: '.claude/commands/specify.md', dest: '.claude/commands/sdd-specify.md' },
            { source: '.claude/commands/plan.md', dest: '.claude/commands/sdd-plan.md' },
            { source: '.claude/commands/tasks.md', dest: '.claude/commands/sdd-tasks.md' },
        ],
        scripts: [
            'scripts/create-new-feature.sh',
            'scripts/setup-plan.sh',
            'scripts/check-task-prerequisites.sh',
            'scripts/common.sh',
            'scripts/get-feature-paths.sh',
            'scripts/update-agent-context.sh',
        ],
        templates: [
            'templates/spec-template.md',
            'templates/plan-template.md',
            'templates/tasks-template.md',
            'templates/agent-file-template.md',
        ],
        memory: [
            'memory/constitution.md',
        ],
    };

    const exists = (p) => fs.existsSync(p);
    const src = (rel) => path.join(specKitRoot, rel);
    const dst = (rel) => path.join(projectRoot, rel);

    if (!exists(specKitRoot)) {
        throw new Error(`[SDD] Spec Kit path not found: ${specKitRoot}`);
    }

    console.log(`\n⠋ Installing Spec Kit (SDD) assets from: ${specKitRoot}`);

    // Ensure directories
    const ensureParent = (p) => fs.mkdirSync(path.dirname(p), { recursive: true });
    const copyFileIdempotent = (from, to, makeExecutable = false) => {
        ensureParent(to);
        if (exists(to)) {
            const a = fs.readFileSync(from);
            const b = fs.readFileSync(to);
            if (Buffer.compare(a, b) === 0) {
                return 'skipped';
            }
            fs.copyFileSync(to, `${to}.bak`);
        }
        fs.copyFileSync(from, to);
        if (makeExecutable) {
            try { fs.chmodSync(to, 0o755); } catch {}
        }
        return 'copied';
    };

    // Commands (with renaming)
    for (const cmd of requiredFiles.claudeCommands) {
        const sourcePath = cmd.source;
        const destPath = cmd.dest;
        const from = src(`.${path.sep}${sourcePath.split('/').slice(1).join(path.sep)}`); // map .claude/commands under spec-kit root
        const to = dst(destPath);
        if (!exists(from)) {
            // In spec-kit, commands live at .claude/commands
            const alt = src(sourcePath);
            copyFileIdempotent(exists(alt) ? alt : from, to);
        } else {
            copyFileIdempotent(from, to);
        }
    }

    // Scripts (executable)
    for (const rel of requiredFiles.scripts) {
        const from = src(rel);
        const to = dst(rel);
        copyFileIdempotent(from, to, true);
    }

    // Templates
    for (const rel of requiredFiles.templates) {
        const from = src(rel);
        const to = dst(rel);
        copyFileIdempotent(from, to);
    }

    // Memory
    for (const rel of requiredFiles.memory) {
        const from = src(rel);
        const to = dst(rel);
        copyFileIdempotent(from, to);
    }

    // Optional quickstart doc (generated)
    const quickstartPath = dst('docs/sdd-quickstart.md');
    ensureParent(quickstartPath);
    if (!exists(quickstartPath)) {
        const qs = `# Spec-Driven Development (SDD) Quickstart\n\n- Requires: Git, bash (macOS/Linux/WSL).\n- Commands live in \`.claude/commands\` for Claude Code.\n\nWorkflow:\n- /sdd-specify → creates branch + \`specs/###-.../spec.md\`\n- /sdd-plan → fills \`plan.md\` and generates research/data model/contracts/quickstart\n- /sdd-tasks → creates \`tasks.md\` from available docs\n\nRun manually (if needed):\n- \`bash scripts/create-new-feature.sh --json \"My feature\"\`\n- \`bash scripts/setup-plan.sh --json\` (must be on feature branch)\n- \`bash scripts/check-task-prerequisites.sh --json\`\n\nBranch rule: must match \`^[0-9]{3}-\` for /sdd-plan and /sdd-tasks.\n`;
        fs.writeFileSync(quickstartPath, qs);
    }

    console.log('\x1b[32m✓ Installed Spec Kit assets\x1b[0m');
}
