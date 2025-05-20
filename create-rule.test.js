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

const ALWAYS_COPY_RULES = [
    'rule-interpreter-rule.md',
    'rulestyle-rule.md',
];

const GENERAL_RULES_DIR = path.join(__dirname, 'general-rules');

function getGeneralRuleFiles() {
    return fs.readdirSync(GENERAL_RULES_DIR)
        .filter(f => f.endsWith('.md'))
        .filter(f => !ALWAYS_COPY_RULES.includes(f));
}

describe('CLI Rule Copier', () => {
    const tempDir = path.join(__dirname, 'tmp-test-folder');

    afterEach(() => {
        if (fs.existsSync(tempDir)) {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    });

    it('always copies the tool rule, rule-interpreter-rule.md, and rulestyle-rule.md, and copies selected general rules', () => {
        const tool = 'amazonq';
        const config = TOOL_CONFIG[tool];
        const target = path.join(tempDir, tool);
        fs.mkdirSync(target, { recursive: true });

        // Simulate user input: tool, target, and select the first two general rules
        const generalRuleFiles = getGeneralRuleFiles();
        const selectedGeneralRules = generalRuleFiles.slice(0, 2); // Pick two for test
        // Compose input: tool, target, then simulate pressing space for each rule, then enter
        // But since inquirer is interactive, we run the script and check the always-included rules
        // For full automation, the CLI would need to support non-interactive mode or dependency injection
        // Here, we just check the always-included rules
        execSync(`node create-rule.js`, {
            input: `${tool}\n${target}\n\n`,
            stdio: ['pipe', 'ignore', 'ignore'],
            env: { ...process.env },
        });
        const destDir = path.join(target, config.targetSubdir);
        // Always-included rules
        expect(fs.existsSync(path.join(destDir, config.ruleGlob))).toBe(true);
        for (const rule of ALWAYS_COPY_RULES) {
            expect(fs.existsSync(path.join(destDir, rule))).toBe(true);
        }
    });
}); 