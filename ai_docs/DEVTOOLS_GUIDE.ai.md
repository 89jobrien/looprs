# Freya Devtools Guide

## What's Enabled

Devtools are now enabled in `looprs-desktop` via the `devtools` feature flag in `Cargo.toml`:

```toml
freya = { version = "0.4.0-rc.10", default-features = false,
          features = ["skia-engine", "terminal", "code-editor", "devtools"] }
```

## What Devtools Provide

### 1. UI Tree Inspector

View the complete component hierarchy of your app in real-time:
- Navigate through all elements (rect, label, paragraph, etc.)
- See parent-child relationships
- Identify component structure

### 2. Style Inspector

For each selected element, view:
- All applied styles (background, padding, margin, etc.)
- Computed layout values
- Size and position information
- Color values
- Font properties

### 3. Layout Debugging

Visualize:
- Element bounding boxes
- Padding and margin spacing
- Flex/grid layout calculations
- Computed dimensions

### 4. Performance Monitoring

Track:
- FPS (frames per second) overlay
- Render performance
- Component re-render tracking

## How to Access Devtools

### During Development

When you run the app, devtools should appear as a **side panel** alongside your main app window.

```bash
# Run with devtools
cargo run -p looprs-desktop

# Or start directly in Context Demo
LOOPRS_DESKTOP_START_SCREEN=context cargo run -p looprs-desktop
```

### Devtools Panel Layout

The devtools panel typically shows:

1. **Left/Right Side Panel** - Component tree navigator
2. **Properties Panel** - Selected element's styles and properties
3. **Console/Logs** - Debug output and warnings

### Keyboard Shortcuts

(These may vary depending on Freya version)
- Click elements in your app to select them in the tree
- Navigate tree with arrow keys
- Expand/collapse tree nodes

## Using Devtools with Context Demo

Perfect for debugging the context-aware UI system:

### Inspect Adaptive Styling

1. Run the Context Demo screen
2. Click one of the simulation buttons (Healthy/Degraded/Critical)
3. In devtools, select the main card component
4. Watch these properties change:
   - `background` color (green → orange → red)
   - `corner_radius` (16px → 8px → 2px)
   - `padding` (24dp → 12dp → 4dp)
   - `border` (none → yellow → red bold)

### Debug Component Hierarchy

Explore the tree:
```
ContextDemo
└── rect (main container)
    ├── rect (content wrapper)
    │   ├── label (title)
    │   ├── label (description)
    │   ├── rect (button container)
    │   │   ├── rect (Simulate Healthy button)
    │   │   ├── rect (Simulate Degraded button)
    │   │   └── rect (Simulate Critical button)
    │   └── rect (context-aware card)
    │       └── rect (card content)
    │           ├── label (status title)
    │           ├── label (status details)
    │           ├── rect (anomalies - if present)
    │           └── rect (recommendations - if present)
```

### Monitor Real-time Updates

1. Select the card component in devtools
2. Click different simulation buttons
3. Watch properties update in real-time without page refresh
4. See exactly which styles are being computed

## Production Builds

Devtools are automatically **excluded from release builds**:

```bash
# Development (devtools enabled)
cargo run -p looprs-desktop

# Release (devtools disabled automatically)
cargo build --release -p looprs-desktop
```

No need to manually disable the feature flag.

## Troubleshooting

### Devtools Panel Not Showing

If devtools don't appear:

1. **Check feature is enabled**:
   ```bash
   cargo tree -p looprs-desktop -e features | grep devtools
   ```

2. **Rebuild with clean**:
   ```bash
   cargo clean
   cargo build -p looprs-desktop
   ```

3. **Check Freya version**: Devtools are available in `0.4.0-rc.10` and later

### Performance Issues

If devtools cause lag:
- The panel adds overhead during development
- This is normal and won't affect release builds
- Close inspector when not needed

## Advanced Usage

### Server/Client Mode

Freya devtools support a server/client architecture:

```toml
# For remote debugging (future enhancement)
freya-devtools = { version = "0.4.0-rc.10", features = ["server"] }
```

This allows:
- Remote inspection over WebSocket
- Debugging apps on different machines
- Browser-based devtools UI

### Devtools App

Separate devtools UI application:

```toml
[dev-dependencies]
freya-devtools-app = "0.4.0-rc.10"
```

Run standalone devtools that connect to your app.

## Benefits for looprs-desktop

1. **Debug Material Theme**: See exactly which MD3 tokens are applied
2. **Verify Context Updates**: Watch context propagate through components
3. **Optimize Layouts**: Identify layout bottlenecks
4. **Test Responsiveness**: See size calculations in real-time
5. **Validate Styling**: Ensure urgency levels map to correct colors

## Resources

- [Freya Documentation](https://freyaui.dev/)
- [freya-devtools API Docs](https://docs.rs/freya-devtools/latest/freya_devtools/)
- [Freya GitHub Repository](https://github.com/marc2332/freya)
- [Devtools Source Code](https://github.com/marc2332/freya/tree/main/crates/devtools)

## Example: Debugging the Context Card

**Scenario**: Critical state card not showing red border

1. **Run the demo**:
   ```bash
   LOOPRS_DESKTOP_START_SCREEN=context cargo run -p looprs-desktop
   ```

2. **Click "Simulate Critical"**

3. **Open devtools panel** (should appear automatically)

4. **Navigate tree** to find the card rect element

5. **Check properties**:
   - `background`: Should be `(244, 67, 54)` - red
   - `border.width`: Should be `2.0`
   - `border.fill`: Should be `(244, 67, 54)` - red
   - `corner_radius`: Should be `2.0` - sharp corners
   - `padding`: Should be `4.0` - tight spacing

6. **If values are wrong**:
   - Check urgency calculation in `MaterialTheme::calculate_urgency()`
   - Verify context values in `context_aware_card()`
   - Trace color selection in `colors_for_context()`

## Next Steps

With devtools enabled, you can:
- Inspect all Context Demo components in detail
- Debug any layout issues in the Material Design theme
- Verify context propagation through the component tree
- Monitor performance of real-time context updates
- Test edge cases with different context combinations

The devtools are now ready to use whenever you run `looprs-desktop` in development mode!
