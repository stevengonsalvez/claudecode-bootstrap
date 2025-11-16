---
name: webapp-testing
description: Toolkit for interacting with and testing local web applications using Playwright. Supports verifying frontend functionality, debugging UI behavior, capturing browser screenshots, and viewing browser logs.
license: Complete terms in LICENSE.txt
---

# Web Application Testing

To test local web applications, write native Python Playwright scripts.

**Helper Scripts Available**:
- `scripts/with_server.py` - Manages server lifecycle (supports multiple servers)

**Always run scripts with `--help` first** to see usage. DO NOT read the source until you try running the script first and find that a customized solution is abslutely necessary. These scripts can be very large and thus pollute your context window. They exist to be called directly as black-box scripts rather than ingested into your context window.

## Decision Tree: Choosing Your Approach

```
User task → Is it static HTML?
    ├─ Yes → Read HTML file directly to identify selectors
    │         ├─ Success → Write Playwright script using selectors
    │         └─ Fails/Incomplete → Treat as dynamic (below)
    │
    └─ No (dynamic webapp) → Is the server already running?
        ├─ No → Run: python scripts/with_server.py --help
        │        Then use the helper + write simplified Playwright script
        │
        └─ Yes → Reconnaissance-then-action:
            1. Navigate and wait for networkidle
            2. Take screenshot or inspect DOM
            3. Identify selectors from rendered state
            4. Execute actions with discovered selectors
```

## Example: Using with_server.py

To start a server, run `--help` first, then use the helper:

**Single server:**
```bash
python scripts/with_server.py --server "npm run dev" --port 3000 -- python your_automation.py
```

**Multiple servers (e.g., backend + frontend):**
```bash
python scripts/with_server.py \
  --server "cd backend && python server.py" --port 8000 \
  --server "cd frontend && npm run dev" --port 3000 \
  -- python your_automation.py
```

To create an automation script, include only Playwright logic (servers are managed automatically):
```python
from playwright.sync_api import sync_playwright

APP_PORT = 3000  # Match the port from --port argument

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True) # Always launch chromium in headless mode
    page = browser.new_page()
    page.goto(f'http://localhost:{APP_PORT}') # Server already running and ready
    page.wait_for_load_state('networkidle') # CRITICAL: Wait for JS to execute
    # ... your automation logic
    browser.close()
```

## Reconnaissance-Then-Action Pattern

1. **Inspect rendered DOM**:
   ```python
   page.screenshot(path='/tmp/inspect.png', full_page=True)
   content = page.content()
   page.locator('button').all()
   ```

2. **Identify selectors** from inspection results

3. **Execute actions** using discovered selectors

## Common Pitfall

❌ **Don't** inspect the DOM before waiting for `networkidle` on dynamic apps
✅ **Do** wait for `page.wait_for_load_state('networkidle')` before inspection

## Best Practices

- **Use bundled scripts as black boxes** - To accomplish a task, consider whether one of the scripts available in `scripts/` can help. These scripts handle common, complex workflows reliably without cluttering the context window. Use `--help` to see usage, then invoke directly. 
- Use `sync_playwright()` for synchronous scripts
- Always close the browser when done
- Use descriptive selectors: `text=`, `role=`, CSS selectors, or IDs
- Add appropriate waits: `page.wait_for_selector()` or `page.wait_for_timeout()`

## Utility Modules

The skill now includes comprehensive utilities for common testing patterns:

### UI Interactions (`utils/ui_interactions.py`)

Handle common UI patterns automatically:

```python
from utils.ui_interactions import (
    dismiss_cookie_banner,
    dismiss_modal,
    click_with_header_offset,
    force_click_if_needed,
    wait_for_no_overlay,
    wait_for_stable_dom
)

# Dismiss cookie consent
dismiss_cookie_banner(page)

# Close welcome modal
dismiss_modal(page, modal_identifier="Welcome")

# Click button behind fixed header
click_with_header_offset(page, 'button#submit', header_height=100)

# Try click with force fallback
force_click_if_needed(page, 'button#action')

# Wait for loading overlays to disappear
wait_for_no_overlay(page)

# Wait for DOM to stabilize
wait_for_stable_dom(page)
```

### Smart Form Filling (`utils/form_helpers.py`)

Intelligently handle form variations:

```python
from utils.form_helpers import (
    SmartFormFiller,
    handle_multi_step_form,
    auto_fill_form
)

# Works with both "Full Name" and "First/Last Name" fields
filler = SmartFormFiller()
filler.fill_name_field(page, "Jane Doe")
filler.fill_email_field(page, "jane@example.com")
filler.fill_password_fields(page, "SecurePass123!")
filler.fill_phone_field(page, "+447700900123")
filler.fill_date_field(page, "1990-01-15", field_hint="birth")

# Auto-fill entire form
results = auto_fill_form(page, {
    'email': 'test@example.com',
    'password': 'Pass123!',
    'full_name': 'Test User',
    'phone': '+447700900123',
    'date_of_birth': '1990-01-15'
})

# Handle multi-step forms
steps = [
    {'fields': {'email': 'test@example.com', 'password': 'Pass123!'}, 'checkbox': True},
    {'fields': {'full_name': 'Test User', 'date_of_birth': '1990-01-15'}},
    {'complete': True}
]
handle_multi_step_form(page, steps)
```

### Supabase Testing (`utils/supabase.py`)

Database operations for Supabase-based apps:

```python
from utils.supabase import SupabaseTestClient, quick_cleanup

# Initialize client
client = SupabaseTestClient(
    url="https://project.supabase.co",
    service_key="your-service-role-key",
    db_password="your-db-password"
)

# Create test user
user_id = client.create_user("test@example.com", "password123")

# Create invite code
client.create_invite_code("TEST2024", code_type="general")

# Bypass email verification
client.confirm_email(user_id)

# Cleanup after test
client.cleanup_related_records(user_id)
client.delete_user(user_id)

# Quick cleanup helper
quick_cleanup("test@example.com", "db_password", "https://project.supabase.co")
```

### Advanced Wait Strategies (`utils/wait_strategies.py`)

Better alternatives to simple sleep():

```python
from utils.wait_strategies import (
    wait_for_api_call,
    wait_for_element_stable,
    smart_navigation_wait,
    combined_wait
)

# Wait for specific API response
response = wait_for_api_call(page, '**/api/profile**')

# Wait for element to stop moving
wait_for_element_stable(page, '.dropdown-menu', stability_ms=1000)

# Smart navigation with URL check
page.click('button#login')
smart_navigation_wait(page, expected_url_pattern='**/dashboard**')

# Comprehensive wait (network + DOM + overlays)
combined_wait(page)
```

## Complete Examples

### Multi-Step Registration

See `examples/multi_step_registration.py` for a complete example showing:
- Database setup (invite codes)
- Cookie banner dismissal
- Multi-step form automation
- Email verification bypass
- Login flow
- Dashboard verification
- Cleanup

Run it:
```bash
python examples/multi_step_registration.py
```

## Using the Webapp-Testing Subagent

A specialized subagent is available for testing automation. Use it to keep your main conversation focused on development:

```
You: "Use webapp-testing agent to register test@example.com and verify the parent role switch works"

Main Agent: [Launches webapp-testing subagent]

Webapp-Testing Agent: [Runs complete automation, returns results]
```

**Benefits:**
- Keeps main context clean
- Specialized for Playwright automation
- Access to all skill utilities
- Automatic screenshot capture
- Clear result reporting

## Reference Files

- **examples/** - Examples showing common patterns:
  - `element_discovery.py` - Discovering buttons, links, and inputs on a page
  - `static_html_automation.py` - Using file:// URLs for local HTML
  - `console_logging.py` - Capturing console logs during automation
  - `multi_step_registration.py` - Complete registration flow example (NEW)

- **utils/** - Reusable utility modules (NEW):
  - `ui_interactions.py` - Cookie banners, modals, overlays, stable waits
  - `form_helpers.py` - Smart form filling, multi-step automation
  - `supabase.py` - Database operations for Supabase apps
  - `wait_strategies.py` - Advanced waiting patterns