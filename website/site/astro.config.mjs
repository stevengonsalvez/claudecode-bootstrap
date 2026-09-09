// @ts-check
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightImageZoom from 'starlight-image-zoom';

// https://astro.build/config
export default defineConfig({
  site: 'https://ainb.app',
  trailingSlash: 'never',
  // Old recall page moved under the dedicated reflect-memory section.
  // The site is served from the apex domain, so there is no base path to
  // prepend here any more.
  redirects: {
    '/docs': '/product/what-is-ainb',
    '/install': '/tui/install',
    '/quickstart': '/tui/quickstart',
    '/concepts': '/product/concepts',
    '/keyboard': '/tui/keyboard-shortcuts',
    '/knowledge/recall-by-example': '/knowledge/reflect-memory/recall',
  },
  // Docs live in the repo-root `docs/` tree (outside this site dir), so MDX
  // there can't resolve `@astrojs/starlight/components` from its own folder.
  // Alias the bare specifier to the package file in this site's node_modules.
  vite: {
    resolve: {
      // Exact-match (end-anchored) so we don't clobber Starlight's own
      // `@astrojs/starlight/components/Banner.astro` etc. - only the bare
      // `@astrojs/starlight/components` specifier used by external docs MDX.
      alias: [
        {
          find: /^@astrojs\/starlight\/components$/,
          replacement: fileURLToPath(
            new URL('./node_modules/@astrojs/starlight/components.ts', import.meta.url)
          ),
        },
      ],
    },
  },
  integrations: [
    starlight({
      title: 'agents-in-a-box',
      description: 'Terminal-native ecosystem for managing AI coding agents.',
      favicon: '/favicon.svg',
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/stevengonsalvez/agents-in-a-box',
        },
      ],
      plugins: [starlightImageZoom()],
      customCss: ['./src/styles/tokens.css', './src/styles/crt.css', './src/styles/reflect-viz.css'],
      editLink: {
        // Starlight builds this as `new URL(baseUrl + entry.filePath)`. The docs
        // collection is loaded from `../../docs`, so `entry.filePath` carries a
        // leading `../../`, and `new URL()` resolves it by popping two segments.
        // Pointing the base at this site's own directory gives it two segments
        // it can afford to lose: `../../` eats `website/site/` and lands on
        // `edit/main/docs/...`. Without the suffix it ate `edit/main/` instead,
        // and every "Edit page" link 404'd.
        baseUrl: 'https://github.com/stevengonsalvez/agents-in-a-box/edit/main/website/site/',
      },
      lastUpdated: true,
      pagination: true,
      head: [
        { tag: 'meta', attrs: { name: 'theme-color', content: '#0F0F18' } },
        { tag: 'link', attrs: { rel: 'preconnect', href: 'https://fonts.googleapis.com' } },
        { tag: 'link', attrs: { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' } },
        {
          tag: 'link',
          attrs: {
            rel: 'stylesheet',
            href: 'https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;700&family=JetBrains+Mono:wght@400;500;700&display=swap',
          },
        },
      ],
      sidebar: [
        {
          label: 'Start here',
          items: [
            { label: 'Overview', slug: 'product/what-is-ainb' },
            { label: 'Install', slug: 'tui/install' },
            { label: 'Quick start', slug: 'tui/quickstart' },
            { label: 'Concepts', slug: 'product/concepts' },
            { label: 'Keyboard', slug: 'tui/keyboard-shortcuts' },
          ],
        },
        {
          label: 'Using ainb',
          items: [
            { label: 'Starting sessions', slug: 'tui/start-session' },
            { label: 'Attaching & tmux', slug: 'tui/attach' },
            { label: 'Code review diff', slug: 'tui/code-review' },
            { label: 'Fleet & multi-agent', slug: 'fleet-bridge' },
            { label: 'ATC background watcher', slug: 'atc-plumbing' },
            { label: 'Shared MCP pool', slug: 'tui/mcp-pool' },
            { label: 'Token optimisation', slug: 'tui/token-optimization' },
            { label: 'Inbox & notifications', slug: 'tui/inbox-notifications' },
            { label: 'Browser dashboard', slug: 'tui/web' },
          ],
        },
        {
          label: 'Configure & Extend',
          collapsed: true,
          items: [
            { label: 'Value proposition', slug: 'product/value' },
            {
              label: 'Plugins',
              items: [
                { label: 'Overview', slug: 'plugins/overview' },
                { label: 'User guide', slug: 'plugins/user-guide' },
                {
                  label: 'In-tree plugins',
                  collapsed: true,
                  items: [
                    { label: 'burndown', slug: 'plugins/burndown' },
                    { label: 'session-reader', slug: 'plugins/session-reader' },
                    { label: 'witr', slug: 'plugins/witr' },
                    { label: 'learnings', slug: 'plugins/learnings' },
                    { label: 'abtop', slug: 'plugins/abtop' },
                  ],
                },
                { label: 'Changelog', slug: 'plugins/changelog' },
              ],
            },
            {
              label: 'Skill manager',
              items: [
                { label: 'Guide & demos', slug: 'skill-manager/guide' },
                {
                  label: 'Commands',
                  collapsed: true,
                  items: [
                    { label: 'Discovery & import', slug: 'skill-manager/discovery' },
                    { label: 'Catalog browse', slug: 'skill-manager/browse' },
                    { label: 'Sync', slug: 'skill-manager/sync' },
                    { label: 'Drift check', slug: 'skill-manager/check' },
                    { label: 'Usage tracking', slug: 'skill-manager/usage' },
                    { label: 'Promote', slug: 'skill-manager/promote' },
                    { label: 'Sandbox testing', slug: 'skill-manager/sandbox-testing' },
                  ],
                },
              ],
            },
            {
              label: 'Toolkit',
              items: [
                { label: 'Overview', slug: 'toolkit/overview' },
                { label: 'Skills', slug: 'toolkit/skills' },
                { label: 'Agents', slug: 'toolkit/agents' },
                { label: 'Bootstrap engine', slug: 'toolkit/bootstrap' },
                {
                  label: 'Claude Code plugins',
                  collapsed: true,
                  items: [
                    { label: 'Overview', slug: 'toolkit/plugins/overview' },
                    { label: 'reflect', slug: 'toolkit/plugins/reflect' },
                    { label: 'ainb-fleet', slug: 'toolkit/plugins/ainb-fleet' },
                    { label: 'ainb-hooks', slug: 'toolkit/plugins/ainb-hooks' },
                  ],
                },
              ],
            },
            {
              label: 'Reflect memory',
              items: [
                { label: 'Overview', slug: 'knowledge/overview' },
                { label: 'Memory browser (serve)', slug: 'knowledge/reflect-memory/serve' },
                { label: 'Hooks & platform', slug: 'knowledge/hooks-and-platform' },
                { label: 'reflect CLI', slug: 'knowledge/reflect-cli' },
              ],
            },
          ],
        },
        {
          label: 'Reference',
          collapsed: true,
          items: [
            { label: 'CLI reference', slug: 'tui/cli' },
            {
              label: 'Plugin authoring & ABI',
              collapsed: true,
              items: [
                { label: 'Authoring guide', slug: 'plugins/authoring' },
                { label: 'Wire spec v2', slug: 'plugins/spec-v2' },
                { label: 'Disambiguation', slug: 'plugins/readme' },
              ],
            },
            { label: 'Fleet cost rollups', slug: 'tui/fleet-cost' },
            {
              label: 'Observability & Telemetry',
              collapsed: true,
              items: [
                { label: 'Overview', slug: 'observability/overview' },
                { label: 'OpenTelemetry to Grafana', slug: 'reference/otel-grafana' },
              ],
            },
            {
              label: 'Architecture & Repos',
              collapsed: true,
              items: [
                { label: 'Architecture deep-dive', slug: 'reference/architecture' },
                { label: 'TUI host architecture', slug: 'tui/architecture' },
                { label: 'Monorepo architecture', slug: 'product/architecture' },
                { label: 'Repositories map', slug: 'reference/repositories' },
                { label: 'Hangar control center', slug: 'hangar/architecture' },
              ],
            },
            {
              label: 'Reflect memory deep-dive',
              collapsed: true,
              items: [
                { label: 'Problem & fit', slug: 'knowledge/reflect-memory/problem-and-fit' },
                { label: 'The construct', slug: 'knowledge/reflect-memory/construct' },
                { label: 'Recall reference (57 ports)', slug: 'knowledge/reflect-memory/recall' },
                { label: 'Why build, not adopt', slug: 'knowledge/reflect-memory/comparison' },
              ],
            },
          ],
        },
        {
          label: 'Help',
          collapsed: true,
          items: [
            { label: 'Troubleshooting & FAQ', slug: 'tui/faq' },
            { label: 'Daemons overlay', slug: 'tui/daemons' },
            {
              label: 'Contributing',
              collapsed: true,
              items: [
                { label: 'Building', slug: 'contributing/building' },
                { label: 'CI / CD', slug: 'contributing/ci-cd' },
                { label: 'Release process', slug: 'contributing/release-process' },
                { label: 'Verifying on a loaded box', slug: 'contributing/verifying-on-a-loaded-box' },
              ],
            },
            { label: 'Glossary', slug: 'reference/glossary' },
          ],
        },
      ],
    }),
  ],
});
