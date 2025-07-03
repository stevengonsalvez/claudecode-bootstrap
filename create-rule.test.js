const fs = require('fs');
const path = require('path');
const os = require('os');
const { execSync } = require('child_process');

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

const ALWAYS_COPY_RULES = [
    'rule-interpreter-rule.md',
    'rulestyle-rule.md',
];

describe('CLI Rule Copier', () => {
    const tempDir = path.join(__dirname, 'tmp-test-folder');
    const homeDir = os.homedir();

    afterEach(() => {
        if (fs.existsSync(tempDir)) {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
        const geminiDir = path.join(homeDir, '.gemini');
        if (fs.existsSync(geminiDir)) {
            fs.rmSync(geminiDir, { recursive: true, force: true });
        }
    });

    it('always copies the tool rule and default rules in non-interactive mode', () => {
        const tool = 'amazonq';
        const config = TOOL_CONFIG[tool];
        const target = path.join(tempDir, tool);
        fs.mkdirSync(target, { recursive: true });

        const command = `node create-rule.js --tool=${tool} --targetFolder=${target}`;
        execSync(command, {
            stdio: 'pipe',
            env: { ...process.env },
        });

        const destDir = path.join(target, config.targetSubdir);
        expect(fs.existsSync(path.join(destDir, config.ruleGlob))).toBe(true);
        for (const rule of ALWAYS_COPY_RULES) {
            expect(fs.existsSync(path.join(destDir, rule))).toBe(true);
        }
    });

    it('copies the entire gemini folder to the home directory in non-interactive mode', () => {
        const tool = 'gemini';
        const config = TOOL_CONFIG[tool];
        const destDir = path.join(homeDir, config.targetSubdir);

        if (fs.existsSync(destDir)) {
            fs.rmSync(destDir, { recursive: true, force: true });
        }

        const command = `node create-rule.js --tool=${tool}`;
        execSync(command, {
            stdio: 'pipe',
            env: { ...process.env },
        });

        expect(fs.existsSync(destDir)).toBe(true);
        expect(fs.existsSync(path.join(destDir, 'GEMINI.md'))).toBe(true);
    });
});