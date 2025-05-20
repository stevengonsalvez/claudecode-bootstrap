const fs = require('fs');
const path = require('path');
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
        targetSubdir: '.cline/rules',
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

describe('CLI Rule Copier', () => {
    const tempDir = path.join(__dirname, 'tmp-test-folder');

    afterEach(() => {
        if (fs.existsSync(tempDir)) {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    });

    for (const tool of Object.keys(TOOL_CONFIG)) {
        it(`copies the ${tool} rule to the correct location in a new project`, () => {
            const config = TOOL_CONFIG[tool];
            const target = path.join(tempDir, tool);
            // Simulate user input using expect prompts
            // Instead, run the script with env vars or a wrapper for automation
            // For simplicity, we just invoke the script and check the result
            // In a real test, use a library like 'expect' or 'inquirer-test'
            fs.mkdirSync(target, { recursive: true });
            execSync(`node create-rule.js`, {
                input: `${tool}\n${target}\n \n`,
                stdio: ['pipe', 'ignore', 'ignore'],
                env: { ...process.env },
            });
            const destRule = path.join(target, config.targetSubdir, config.ruleGlob);
            expect(fs.existsSync(destRule)).toBe(true);
            const ruleContent = fs.readFileSync(destRule, 'utf8');
            expect(ruleContent.length).toBeGreaterThan(0);
        });
    }
}); 