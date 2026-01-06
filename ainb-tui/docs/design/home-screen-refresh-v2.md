# AINB-TUI Home Screen Refresh Design

**Version:** 2.0
**Date:** 2026-01-06
**Designer:** UI Design Agent

---

## 1. Mascot Concept: "Boxy" - The Agent Cube

### Concept Description

"Boxy" is a friendly AI cube character that embodies the "Agents in a Box" theme. The design uses:
- A 3D isometric cube with a friendly face
- Glowing circuit-like patterns suggesting AI/tech
- Subtle "breathing" animation (pulsing glow effect)
- The cube appears to float/hover with a shadow underneath

The character is approachable yet tech-forward, working well in ASCII art constraints while feeling modern and memorable.

---

## 2. ASCII Art Mascot - Static and Animated Frames

### Static Version (Compact - 7 lines, fits sidebar or corner)

```
    ╭──────────╮
   ╱│          │╲
  ╱ │  ◉    ◉  │ ╲
 ╱  │    ──    │  ╲
│   │          │   │
│   ╰──────────╯   │
╰──────────────────╯
```

### Static Version (Full - Hero Section, 11 lines)

```
         ╭────────────────────╮
        ╱│                    │╲
       ╱ │    ◉          ◉    │ ╲
      ╱  │                    │  ╲
     ╱   │       ╭────╮       │   ╲
    │    │       ╰────╯       │    │
    │    │                    │    │
    │    │   ┌──┐      ┌──┐   │    │
    │    ╰───┴──┴──────┴──┴───╯    │
    ╰──────────────────────────────╯
           ░░░░░░░░░░░░░░░░░
```

### Animation Frames (4-frame breathing/pulse cycle)

**Frame 1 - Neutral:**
```
    ╭──────────╮
   ╱│          │╲
  ╱ │  ◉    ◉  │ ╲
 ╱  │    ──    │  ╲
│   │          │   │
│   ╰──────────╯   │
╰──────────────────╯
      ░░░░░░░░
```

**Frame 2 - Eyes Blink:**
```
    ╭──────────╮
   ╱│          │╲
  ╱ │  ─    ─  │ ╲
 ╱  │    ──    │  ╲
│   │          │   │
│   ╰──────────╯   │
╰──────────────────╯
      ░░░░░░░░
```

**Frame 3 - Slight Bounce Up:**
```

    ╭──────────╮
   ╱│          │╲
  ╱ │  ◉    ◉  │ ╲
 ╱  │    ──    │  ╲
│   │          │   │
│   ╰──────────╯   │
╰──────────────────╯
       ░░░░░░
```

**Frame 4 - Happy Expression:**
```
    ╭──────────╮
   ╱│          │╲
  ╱ │  ◉    ◉  │ ╲
 ╱  │    ◡◡    │  ╲
│   │          │   │
│   ╰──────────╯   │
╰──────────────────╯
      ░░░░░░░░
```

### Alternative: Minimal Sidebar Icon (3 lines)

```
╭─◉─◉─╮
│ ─── │
╰─────╯
```

### Color Mapping for Boxy

| Element | Color | Hex |
|---------|-------|-----|
| Cube outline | CORNFLOWER_BLUE | RGB(100, 149, 237) |
| Eyes (normal) | GOLD | RGB(255, 215, 0) |
| Eyes (blink) | MUTED_GRAY | RGB(120, 120, 140) |
| Mouth | SOFT_WHITE | RGB(220, 220, 230) |
| Shadow | MUTED_GRAY (dim) | RGB(80, 80, 100) |
| Glow effect | SELECTION_GREEN | RGB(100, 200, 100) |

---

## 3. Sidebar Button Layout Design

### Design Philosophy

Following familiar patterns from VS Code, Discord, and Slack:
- Vertical icon-based navigation on the left edge
- Active state with highlight bar indicator
- Tooltip-style labels on hover (or inline for TUI)
- Grouped by function with visual separators

### Sidebar Layout (Width: 4-6 columns for icons, or 20 columns with labels)

```
┌──────────────────────────────────────────────────────────────┐
│╭────╮                                                        │
││    │  AINB - Agents in a Box                               │
│╰────╯                                                        │
├────────────────────────────────────────────────────────────────
│                                                              │
│ ▌  Sessions     │                                           │
│    ────────────  │                                           │
│ ○  New Agent    │                                           │
│ ○  Active (3)    │              [MAIN CONTENT AREA]         │
│ ○  History       │                                           │
│                  │                                           │
│ ▌  Tools        │                                           │
│    ────────────  │                                           │
│ ○  Git           │                                           │
│ ○  Catalog       │                                           │
│ ○  Config        │                                           │
│                  │                                           │
│ ────────────────│                                           │
│ ○  Help          │                                           │
│ ○  Quit          │                                           │
└──────────────────────────────────────────────────────────────┘
```

### Icon-Only Mode (Compact Sidebar - 4 columns)

```
╭──╮
│ │  <- Home
│ │  <- Sessions
│ │  <- New
├──┤
│ │  <- Git
│ │  <- Catalog
│ │  <- Config
├──┤
│?│  <- Help
│×│  <- Quit
╰──╯
```

### Button States

**Default:**
```
│ ○  Sessions │
```

**Selected/Active:**
```
│▌   Sessions │  <- Green accent bar + highlight background
```

**Hover (if cursor-based):**
```
│   Sessions │  <- Subtle background change
```

### Navigation Sections

| Section | Items | Shortcut |
|---------|-------|----------|
| **Home** | Home Dashboard | `q` |
| **Sessions** | New Agent, Active Sessions, History | `n`, `s`, `h` |
| **Tools** | Git View, Catalog, Config | `g`, `c`, `C` |
| **System** | Help, Quit | `?`, `Q` |

---

## 4. Full Home Screen Layout

### Terminal Dimensions: 120 columns x 40 rows (optimal)

```
╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│                                                                                                                        │
│   ╭────╮    A I N B  ─  Agents in a Box                                                            v2.0.0  │
│   │◉  ◉│    ──────────────────────────────────                                                                         │
│   │ ── │    Your AI-Powered Development Hub                                                                           │
│   ╰────╯                                                                                                               │
│                                                                                                                        │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                                        │
│  ╭──────────────╮   QUICK ACTIONS                                                                                      │
│  │              │   ━━━━━━━━━━━━━━                                                                                     │
│  │ ▌  Home     │                                                                                                      │
│  │    Sessions  │   ╭─────────────────────────╮  ╭─────────────────────────╮  ╭─────────────────────────╮             │
│  │    New       │   │                         │  │                         │  │                         │             │
│  │              │   │    New Agent Session   │  │    Active Sessions     │  │    Git Operations       │             │
│  │ ───────────  │   │         [n]             │  │         [s]             │  │         [g]             │             │
│  │              │   │                         │  │                         │  │                         │             │
│  │ ▌  Tools    │   │    Start a new Claude   │  │    View and manage     │  │    Commits, branches    │             │
│  │    Git       │   │    coding session       │  │    running agents       │  │    and worktrees        │             │
│  │    Catalog   │   │                         │  │                         │  │                         │             │
│  │    Config    │   ╰─────────────────────────╯  ╰─────────────────────────╯  ╰─────────────────────────╯             │
│  │              │                                                                                                      │
│  │ ───────────  │   ╭─────────────────────────╮  ╭─────────────────────────╮  ╭─────────────────────────╮             │
│  │              │   │                         │  │                         │  │                         │             │
│  │    Help      │   │    Skill Catalog       │  │    Configuration        │  │    Session Stats        │             │
│  │    Quit      │   │         [c]             │  │         [C]             │  │         [i]             │             │
│  │              │   │                         │  │                         │  │                         │             │
│  ╰──────────────╯   │    Browse and manage   │  │    API keys, themes    │  │    Usage metrics and    │             │
│                     │    agent skills         │  │    and preferences      │  │    session history      │             │
│                     │                         │  │                         │  │                         │             │
│                     ╰─────────────────────────╯  ╰─────────────────────────╯  ╰─────────────────────────╯             │
│                                                                                                                        │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                                        │
│    Recent Activity                                                                                                    │
│    ────────────────                                                                                                    │
│     my-project/feat-login-flow  Running  2h ago  │   Claude Opus 4.5                                               │
│                                                                                                                        │
├────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│  n new │ s sessions │ g git │ c catalog │ C config │ ? help │ q quit                                                 │
╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
```

### Layout Proportions

```
Terminal Size: 120 cols x 40 rows

┌─────────────────────────────────────────────────────────────────┐
│ Header with Mascot + Title (6 rows)                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│ ┌─────────┐ ┌─────────────────────────────────────────────────┐│
│ │         │ │                                                 ││
│ │ Sidebar │ │         Action Cards Grid (2x3)                ││
│ │  (20%)  │ │              (80%)                              ││
│ │         │ │                                                 ││
│ │         │ │                                                 ││
│ └─────────┘ └─────────────────────────────────────────────────┘│
│                      (Main Content: 26 rows)                    │
├─────────────────────────────────────────────────────────────────┤
│ Recent Activity Bar (3 rows)                                    │
├─────────────────────────────────────────────────────────────────┤
│ Help Bar / Keyboard Shortcuts (2 rows)                          │
└─────────────────────────────────────────────────────────────────┘
```

### Constraint Breakdown (Ratatui)

```rust
// Vertical layout
Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(6),   // Header with mascot
        Constraint::Min(20),     // Main content area
        Constraint::Length(4),   // Recent activity
        Constraint::Length(2),   // Help bar
    ])

// Horizontal layout for main content
Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Length(20),  // Sidebar (fixed width)
        Constraint::Min(60),     // Content area (flexible)
    ])

// Grid for action cards (2 rows x 3 cols)
// Row constraints
Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Percentage(50),  // Row 1
        Constraint::Percentage(50),  // Row 2
    ])

// Column constraints per row
Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Percentage(33),  // Card 1
        Constraint::Percentage(34),  // Card 2
        Constraint::Percentage(33),  // Card 3
    ])
```

---

## 5. Component Styling Details

### Header Block

```rust
Block::default()
    .borders(Borders::BOTTOM)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(CORNFLOWER_BLUE))
    .style(Style::default().bg(DARK_BG))
```

### Sidebar Button (Selected)

```rust
// Selected state
Line::from(vec![
    Span::styled("", Style::default().fg(SELECTION_GREEN)),  // Indicator
    Span::styled("  ", Style::default()),
    Span::styled("", Style::default().fg(GOLD)),  // Icon
    Span::styled("  Sessions", Style::default().fg(SOFT_WHITE).add_modifier(Modifier::BOLD)),
])
.style(Style::default().bg(LIST_HIGHLIGHT_BG))

// Unselected state
Line::from(vec![
    Span::styled("   ", Style::default()),  // No indicator
    Span::styled("", Style::default().fg(MUTED_GRAY)),  // Icon dimmed
    Span::styled("  Sessions", Style::default().fg(MUTED_GRAY)),
])
```

### Action Card

```rust
Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(if selected { SELECTION_GREEN } else { CORNFLOWER_BLUE }))
    .style(Style::default().bg(if selected { LIST_HIGHLIGHT_BG } else { PANEL_BG }))
    .title(Line::from(vec![
        Span::styled("  ", Style::default().fg(GOLD)),  // Icon
        Span::styled("New Agent", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
        Span::styled(" [n]", Style::default().fg(MUTED_GRAY)),  // Shortcut
    ]))
```

### Recent Activity Bar

```rust
Line::from(vec![
    Span::styled("  ", Style::default().fg(GOLD)),
    Span::styled(" Recent: ", Style::default().fg(MUTED_GRAY)),
    Span::styled("my-project", Style::default().fg(SOFT_WHITE).add_modifier(Modifier::BOLD)),
    Span::styled("/", Style::default().fg(MUTED_GRAY)),
    Span::styled("feat-login", Style::default().fg(CORNFLOWER_BLUE)),
    Span::styled("  ", Style::default()),
    Span::styled("", Style::default().fg(SELECTION_GREEN)),  // Running indicator
    Span::styled(" Running", Style::default().fg(SELECTION_GREEN)),
    Span::styled("  ", Style::default()),
    Span::styled("2h ago", Style::default().fg(MUTED_GRAY)),
])
```

### Help Bar

```rust
Line::from(vec![
    Span::styled(" n", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
    Span::styled(" new ", Style::default().fg(MUTED_GRAY)),
    Span::styled("|", Style::default().fg(Color::Rgb(60, 60, 80))),
    Span::styled(" s", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
    Span::styled(" sessions ", Style::default().fg(MUTED_GRAY)),
    Span::styled("|", Style::default().fg(Color::Rgb(60, 60, 80))),
    // ... continue pattern
])
```

---

## 6. Animation Implementation for Ratatui

### Animation State Structure

```rust
pub struct MascotAnimation {
    frames: Vec<Vec<&'static str>>,
    current_frame: usize,
    last_update: std::time::Instant,
    frame_duration: std::time::Duration,
    blink_timer: std::time::Instant,
    blink_interval: std::time::Duration,
}

impl MascotAnimation {
    pub fn new() -> Self {
        Self {
            frames: vec![
                MASCOT_FRAME_NEUTRAL.to_vec(),
                MASCOT_FRAME_BLINK.to_vec(),
                MASCOT_FRAME_BOUNCE.to_vec(),
                MASCOT_FRAME_HAPPY.to_vec(),
            ],
            current_frame: 0,
            last_update: std::time::Instant::now(),
            frame_duration: std::time::Duration::from_millis(150),
            blink_timer: std::time::Instant::now(),
            blink_interval: std::time::Duration::from_secs(4),
        }
    }

    pub fn tick(&mut self) {
        let now = std::time::Instant::now();

        // Random blink every 4-6 seconds
        if now.duration_since(self.blink_timer) > self.blink_interval {
            self.current_frame = 1; // Blink frame
            self.last_update = now;
            self.blink_timer = now;
            // Randomize next blink interval
            self.blink_interval = std::time::Duration::from_secs(
                4 + rand::random::<u64>() % 3
            );
        } else if now.duration_since(self.last_update) > self.frame_duration {
            // Return to neutral after blink
            if self.current_frame == 1 {
                self.current_frame = 0;
            }
            self.last_update = now;
        }
    }

    pub fn get_current_frame(&self) -> &[&'static str] {
        &self.frames[self.current_frame]
    }
}
```

### Frame Definitions

```rust
const MASCOT_FRAME_NEUTRAL: &[&str] = &[
    "    ╭──────────╮",
    "   ╱│          │╲",
    "  ╱ │  ◉    ◉  │ ╲",
    " ╱  │    ──    │  ╲",
    "│   │          │   │",
    "│   ╰──────────╯   │",
    "╰──────────────────╯",
    "      ░░░░░░░░",
];

const MASCOT_FRAME_BLINK: &[&str] = &[
    "    ╭──────────╮",
    "   ╱│          │╲",
    "  ╱ │  ─    ─  │ ╲",
    " ╱  │    ──    │  ╲",
    "│   │          │   │",
    "│   ╰──────────╯   │",
    "╰──────────────────╯",
    "      ░░░░░░░░",
];

const MASCOT_FRAME_BOUNCE: &[&str] = &[
    "",
    "    ╭──────────╮",
    "   ╱│          │╲",
    "  ╱ │  ◉    ◉  │ ╲",
    " ╱  │    ──    │  ╲",
    "│   │          │   │",
    "│   ╰──────────╯   │",
    "╰──────────────────╯",
    "       ░░░░░░",
];

const MASCOT_FRAME_HAPPY: &[&str] = &[
    "    ╭──────────╮",
    "   ╱│          │╲",
    "  ╱ │  ◉    ◉  │ ╲",
    " ╱  │    ◡◡    │  ╲",
    "│   │          │   │",
    "│   ╰──────────╯   │",
    "╰──────────────────╯",
    "      ░░░░░░░░",
];
```

### Rendering the Mascot with Colors

```rust
fn render_mascot(&self, frame: &mut Frame, area: Rect, mascot: &MascotAnimation) {
    let mascot_lines = mascot.get_current_frame();

    let styled_lines: Vec<Line> = mascot_lines.iter().map(|line| {
        let mut spans = Vec::new();

        for ch in line.chars() {
            let style = match ch {
                '◉' => Style::default().fg(GOLD),
                '─' | '◡' => Style::default().fg(SOFT_WHITE),
                '░' => Style::default().fg(Color::Rgb(60, 60, 80)),
                '╭' | '╮' | '╯' | '╰' | '│' | '╱' | '╲' => {
                    Style::default().fg(CORNFLOWER_BLUE)
                }
                _ => Style::default().fg(SOFT_WHITE),
            };
            spans.push(Span::styled(ch.to_string(), style));
        }

        Line::from(spans)
    }).collect();

    let mascot_widget = Paragraph::new(styled_lines)
        .alignment(Alignment::Left);

    frame.render_widget(mascot_widget, area);
}
```

---

## 7. Responsive Layout Considerations

### Minimum Size (80 x 24)

For smaller terminals, switch to compact mode:
- Hide sidebar labels (icons only)
- Reduce cards to 2x2 or list view
- Shrink mascot to mini version

```rust
fn get_layout_mode(area: Rect) -> LayoutMode {
    match (area.width, area.height) {
        (w, h) if w >= 120 && h >= 35 => LayoutMode::Full,
        (w, h) if w >= 100 && h >= 30 => LayoutMode::Standard,
        (w, h) if w >= 80 && h >= 24 => LayoutMode::Compact,
        _ => LayoutMode::Minimal,
    }
}
```

### Compact Mode Layout (80 cols)

```
╭──────────────────────────────────────────────────────────────────────────────╮
│ ╭─◉◉─╮  AINB - Agents in a Box                                   v2.0.0    │
│ │ ── │                                                                      │
│ ╰────╯                                                                      │
├──────────────────────────────────────────────────────────────────────────────┤
│   ╭────────────────────╮  ╭────────────────────╮  ╭────────────────────╮   │
│   │ New Agent     [n] │  │ Sessions      [s] │  │ Git           [g] │   │
│   │ Start new session  │  │ Manage active     │  │ Commits, branches │   │
│   ╰────────────────────╯  ╰────────────────────╯  ╰────────────────────╯   │
│   ╭────────────────────╮  ╭────────────────────╮  ╭────────────────────╮   │
│   │ Catalog       [c] │  │ Config        [C] │  │ Stats         [i] │   │
│   │ Browse skills      │  │ Settings          │  │ Usage metrics     │   │
│   ╰────────────────────╯  ╰────────────────────╯  ╰────────────────────╯   │
├──────────────────────────────────────────────────────────────────────────────┤
│  my-project/feat-login  Running                                            │
├──────────────────────────────────────────────────────────────────────────────┤
│ n new | s sessions | g git | c catalog | C config | ? help | q quit         │
╰──────────────────────────────────────────────────────────────────────────────╯
```

---

## 8. Navigation State Management

### Home Screen State

```rust
pub struct HomeScreenState {
    /// Currently focused area: Sidebar or Grid
    pub focus_area: FocusArea,

    /// Selected sidebar item index
    pub sidebar_index: usize,

    /// Selected grid position (row, col)
    pub grid_position: (usize, usize),

    /// Animation state for mascot
    pub mascot_animation: MascotAnimation,

    /// Recent session for quick resume
    pub recent_session: Option<RecentSession>,
}

pub enum FocusArea {
    Sidebar,
    Grid,
}

pub struct RecentSession {
    pub workspace: String,
    pub branch: String,
    pub status: SessionStatus,
    pub last_active: chrono::DateTime<chrono::Utc>,
    pub model: String,
}
```

### Keyboard Navigation

| Key | Action |
|-----|--------|
| `Tab` | Toggle focus between Sidebar and Grid |
| `j` / `Down` | Move down in current focus area |
| `k` / `Up` | Move up in current focus area |
| `h` / `Left` | Move left (grid only) |
| `l` / `Right` | Move right (grid only) |
| `Enter` | Activate selected item |
| `n` | Quick: New Agent Session |
| `s` | Quick: Sessions View |
| `g` | Quick: Git View |
| `c` | Quick: Catalog |
| `C` | Quick: Config |
| `?` | Help overlay |
| `q` / `Esc` | Quit / Back |

---

## 9. Color Token Updates

### Suggested Palette Enhancements

```rust
// Existing (keep)
const CORNFLOWER_BLUE: Color = Color::Rgb(100, 149, 237);
const GOLD: Color = Color::Rgb(255, 215, 0);
const SELECTION_GREEN: Color = Color::Rgb(100, 200, 100);
const DARK_BG: Color = Color::Rgb(25, 25, 35);
const PANEL_BG: Color = Color::Rgb(30, 30, 40);
const SOFT_WHITE: Color = Color::Rgb(220, 220, 230);
const MUTED_GRAY: Color = Color::Rgb(120, 120, 140);

// New additions
const ACCENT_PURPLE: Color = Color::Rgb(160, 120, 200);  // For special highlights
const HOVER_BG: Color = Color::Rgb(35, 35, 50);          // Subtle hover state
const CARD_BORDER: Color = Color::Rgb(70, 80, 100);      // Softer card borders
const SHADOW_COLOR: Color = Color::Rgb(15, 15, 20);      // Drop shadow effect
const SUCCESS_GREEN: Color = Color::Rgb(80, 180, 80);    // Running status
const WARNING_AMBER: Color = Color::Rgb(220, 160, 40);   // Idle/warning status
const ERROR_RED: Color = Color::Rgb(200, 80, 80);        // Stopped/error status
```

---

## 10. Implementation Checklist

### Phase 1: Core Structure
- [ ] Create `MascotAnimation` struct and frame definitions
- [ ] Update `HomeScreenComponent` with new layout
- [ ] Implement sidebar component
- [ ] Add focus state management

### Phase 2: Visual Polish
- [ ] Apply color tokens to all components
- [ ] Add selection indicators and hover states
- [ ] Implement card grid with proper spacing
- [ ] Add recent activity bar

### Phase 3: Animation
- [ ] Integrate mascot animation into render loop
- [ ] Add blink timing with randomization
- [ ] Consider subtle card hover animations

### Phase 4: Responsive
- [ ] Implement layout mode detection
- [ ] Create compact mode layouts
- [ ] Test at various terminal sizes

---

## 11. File Structure for Implementation

```
src/components/
  home_screen.rs          # Main home screen component (update)
  sidebar.rs              # New: Sidebar navigation component
  action_card.rs          # New: Reusable action card widget
  mascot.rs               # New: Mascot animation and rendering

src/widgets/
  animated_text.rs        # New: Support for frame-based text animation
```

---

## Summary

This design transforms the AINB-TUI home screen from a basic tile grid into a polished, modern terminal application that:

1. **Features "Boxy"** - a friendly, animated AI cube mascot that adds personality
2. **Uses familiar sidebar navigation** - inspired by VS Code/Discord patterns
3. **Maintains the 2x3 action card grid** - for quick access to all features
4. **Includes contextual information** - recent session bar for quick resume
5. **Remains accessible** - clear keyboard shortcuts, good contrast
6. **Works across terminal sizes** - responsive layouts from 80 to 120+ columns

The design prioritizes rapid implementation while delivering a premium, professional feel that will make AINB stand out in screenshots and social sharing.
