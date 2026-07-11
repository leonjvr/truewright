# popup-auto-attach Specification

## Purpose
Lets an agent test flows where the application under test opens a new top-level browsing context as a side effect of interacting with it -- third-party OAuth login, an external payment processor, a `target="_blank"` link -- by noticing and attaching to the new page automatically and exposing it as something the agent can explicitly discover and switch to, rather than it being invisible to the engine.
## Requirements
### Requirement: Automatic attach to new top-level targets
The engine SHALL automatically attach to a new top-level browsing context (a popup window or a new tab) created as a side effect of interacting with the current page, without requiring an explicit navigation call, and track it alongside the page(s) already known to the session.

#### Scenario: Clicking a link that opens a new tab is noticed
- **WHEN** an action on the current page causes a new top-level target to open (e.g. a `target="_blank"` link or a `window.open()` call)
- **THEN** the new page becomes attached and discoverable via `browser_list_pages`, without the agent needing to do anything beyond the action that triggered it

### Requirement: Explicit page listing and switching
The engine SHALL expose every currently-attached page to an agent via `browser_list_pages`, and SHALL let an agent explicitly select which attached page subsequent actions operate against via `browser_switch_page`. The engine SHALL NOT automatically switch the active page on a new attach.

#### Scenario: Listing shows all attached pages and which is active
- **WHEN** `browser_list_pages` is called after a popup has attached
- **THEN** the result includes every attached page's identifier, URL, and title, with a clear indication of which one is currently active

#### Scenario: Switching changes which page subsequent actions target
- **WHEN** `browser_switch_page` is called with the identifier of an attached page
- **THEN** subsequent `browser_click`/`browser_type`/`browser_snapshot`/etc. calls act against that page instead of the previously active one

#### Scenario: Switching to an unknown page fails clearly
- **WHEN** `browser_switch_page` is called with an identifier that doesn't match any currently-attached page
- **THEN** the call fails with a clear error, rather than silently doing nothing or crashing

### Requirement: Predictable fallback when the active page closes
The engine SHALL fall back to the session's original page if the currently-active page closes (e.g. a popup completing an OAuth redirect and closing itself), rather than leaving the session with no usable active page or an arbitrary one.

#### Scenario: Active popup closing itself falls back to the original page
- **WHEN** the currently-active page (a popup) is closed while it is active
- **THEN** the session's active page reverts to the original page the session was launched with

