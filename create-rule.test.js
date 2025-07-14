const fs = require('fs');
const path = require('path');
const os = require('os');
const { execSync } = require('child_process');

const TOOL_CONFIG = {
    amazonq: {
        ruleGlob: 'q-rulestore-rule.md',
        ruleDir: 'amazonq',
        targetSubdir: '.amazonq/rules',
        mcpFile: 'amazonq/mcp.json',
        mcpTarget: '.amazonq/mcp.json',
        sharedContentDir: 'claude-code',
        copySharedContent: true,
        excludeFiles: ['CLAUDE.md', 'settings.local.json'],
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
        ruleGlob: 'GEMINI.md',
        ruleDir: 'gemini',
        targetSubdir: '.gemini',
        sharedContentDir: 'claude-code',
        copySharedContent: true,
        excludeFiles: ['CLAUDE.md'],
        settingsFile: 'gemini/settings.json',
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

    it('copies shared content to gemini project folder with correct structure', () => {
        const tool = 'gemini';
        const config = TOOL_CONFIG[tool];
        const target = path.join(tempDir, tool);
        fs.mkdirSync(target, { recursive: true });

        const command = `node create-rule.js --tool=${tool} --targetFolder=${target}`;
        execSync(command, {
            stdio: 'pipe',
            env: { ...process.env },
        });

        const destDir = path.join(target, config.targetSubdir);
        expect(fs.existsSync(destDir)).toBe(true);
        expect(fs.existsSync(path.join(destDir, 'GEMINI.md'))).toBe(true);
        expect(fs.existsSync(path.join(destDir, 'commands'))).toBe(true);
        expect(fs.existsSync(path.join(destDir, 'templates'))).toBe(true);
        expect(fs.existsSync(path.join(destDir, 'session', 'current-session.yaml'))).toBe(true);
        // Should NOT have CLAUDE.md
        expect(fs.existsSync(path.join(destDir, 'CLAUDE.md'))).toBe(false);

        // Check commands folder has files
        const commandsDir = path.join(destDir, 'commands');
        const commandFiles = fs.readdirSync(commandsDir);
        expect(commandFiles.length).toBeGreaterThan(0);
        expect(commandFiles.some(f => f.endsWith('.md'))).toBe(true);

        // Check template substitution
        const geminiContent = fs.readFileSync(path.join(destDir, 'GEMINI.md'), 'utf8');
        expect(geminiContent).toContain('.gemini/session/current-session.yaml');
        expect(geminiContent).not.toContain('{{TOOL_DIR}}');
        
        // Check settings.json is copied to .gemini folder
        expect(fs.existsSync(path.join(destDir, 'settings.json'))).toBe(true);
        const settingsContent = fs.readFileSync(path.join(destDir, 'settings.json'), 'utf8');
        const settings = JSON.parse(settingsContent);
        expect(settings.theme).toBe('GitHub');
        expect(settings.mcpServers).toBeDefined();
    });

    it('copies amazonq files with correct structure', () => {
        const tool = 'amazonq';
        const config = TOOL_CONFIG[tool];
        const target = path.join(tempDir, tool);
        fs.mkdirSync(target, { recursive: true });

        const command = `node create-rule.js --tool=${tool} --targetFolder=${target}`;
        execSync(command, {
            stdio: 'pipe',
            env: { ...process.env },
        });

        // Check rules directory
        const rulesDir = path.join(target, config.targetSubdir);
        expect(fs.existsSync(path.join(rulesDir, config.ruleGlob))).toBe(true);
        for (const rule of ALWAYS_COPY_RULES) {
            expect(fs.existsSync(path.join(rulesDir, rule))).toBe(true);
        }

        // Check .amazonq/rules directory structure (shared content is in rules dir)
        expect(fs.existsSync(path.join(rulesDir, 'commands'))).toBe(true);
        expect(fs.existsSync(path.join(rulesDir, 'templates'))).toBe(true);
        expect(fs.existsSync(path.join(rulesDir, 'session', 'current-session.yaml'))).toBe(true);

        // Check commands folder has files
        const commandsDir = path.join(rulesDir, 'commands');
        const commandFiles = fs.readdirSync(commandsDir);
        expect(commandFiles.length).toBeGreaterThan(0);
        expect(commandFiles.some(f => f.endsWith('.md'))).toBe(true);

        // Check that settings.local.json is excluded
        expect(fs.existsSync(path.join(rulesDir, 'settings.local.json'))).toBe(false);

        // Check AmazonQ.md in the rules directory
        expect(fs.existsSync(path.join(rulesDir, 'AmazonQ.md'))).toBe(true);

        // Check for the linked AmazonQ.md in the project root
        const rootAmazonQPath = path.join(target, 'AmazonQ.md');
        expect(fs.existsSync(rootAmazonQPath)).toBe(true);
        const rootAmazonQContent = fs.readFileSync(rootAmazonQPath, 'utf8');
        expect(rootAmazonQContent).toBe('@.amazonq/rules/AmazonQ.md');

        // Check mcp.json is copied to .amazonq folder
        expect(fs.existsSync(path.join(target, '.amazonq', 'mcp.json'))).toBe(true);
        const mcpContent = fs.readFileSync(path.join(target, '.amazonq', 'mcp.json'), 'utf8');
        const mcpConfig = JSON.parse(mcpContent);
        expect(mcpConfig.mcpServers).toBeDefined();
        expect(mcpConfig.mcpServers['container-use']).toBeDefined();

        // Check template substitution
        const amazonqContent = fs.readFileSync(path.join(rulesDir, 'AmazonQ.md'), 'utf8');
        expect(amazonqContent).toContain('.amazonq/session/current-session.yaml');
        expect(amazonqContent).not.toContain('{{TOOL_DIR}}');
    });

    it('claude-code copies to home directory with correct paths', () => {
        const tool = 'claude-code';
        const config = TOOL_CONFIG[tool];
        const mockHomeDir = path.join(tempDir, 'mock-home');
        fs.mkdirSync(mockHomeDir, { recursive: true });
        const destDir = path.join(mockHomeDir, config.targetSubdir);

        const command = `node create-rule.js --tool=${tool} --homeDir=${mockHomeDir}`;
        execSync(command, {
            stdio: 'pipe',
            env: { ...process.env },
        });

        expect(fs.existsSync(destDir)).toBe(true);
        expect(fs.existsSync(path.join(destDir, 'CLAUDE.md'))).toBe(true);
        expect(fs.existsSync(path.join(destDir, 'session', 'current-session.yaml'))).toBe(true);

        // Check commands folder exists and has files
        const commandsDir = path.join(destDir, 'commands');
        expect(fs.existsSync(commandsDir)).toBe(true);
        const commandFiles = fs.readdirSync(commandsDir);
        expect(commandFiles.length).toBeGreaterThan(0);
        expect(commandFiles.some(f => f.endsWith('.md'))).toBe(true);

        // Check templates folder exists
        expect(fs.existsSync(path.join(destDir, 'templates'))).toBe(true);

        // Check template substitution
        const claudeContent = fs.readFileSync(path.join(destDir, 'CLAUDE.md'), 'utf8');
        expect(claudeContent).toContain('.claude/session/current-session.yaml');
        expect(claudeContent).toContain('~/.claude/docs/');
        expect(claudeContent).not.toContain('{{TOOL_DIR}}');
        expect(claudeContent).not.toContain('{{HOME_TOOL_DIR}}');

        // Check that settings.local.json is excluded
        expect(fs.existsSync(path.join(destDir, 'settings.local.json'))).toBe(false);
    });
});