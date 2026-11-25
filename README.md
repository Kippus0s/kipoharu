# Kipoharu Lite 0.92K

A workflow-focused fork of Koharu v0.92.0, with quality-of-life and productivity improvements for manga editing, cleaning and typesetting.

Kipoharu branched away from upstream following the introduction of Gallery/Reader features, and has removed the AI agent that arrived at a similar time. 
The intention is to keep the application resilient to featurecreep, and focused on translating and editing individual comics, rather than expanding into manga browsing, reading and general-purpose AI assistance.

Selected upstream bug fixes stability fixes have also been incorporated since Koharu moved on from 0.92.

## Changes

### Layer Ordering and Cleaning

A program tailor-made for working on manga and comics should make text boxes safe from the tools included for cleaning purposes.

- New Paint and Remove layers are automatically placed below panel and text layers.
- New cleaning rasters are placed above existing cleaning rasters.
- Newly created Remove rasters are automatically selected.
- Remove can reuse the selected raster instead of creating a new layer for every application.
- Brush and Remove can work together on the same raster.
- Additional raster layers can still be created manually.

Cleaning and redrawing now work much more like it did in older versions of Koharu, without giving up the improvements made since then.




### Typesetting and Text Fitting

The TypeBar includes a drop-down for selecting how text fits within its designated area.

- **Fixed:** Preserves the specified font size.
- **Shrink:** Reduces the font size when necessary.
- **Grow:** Expands a rectangular text area to accommodate its contents.
- **Best Fit:** Automatically adjusts the font size to fit the available area.
- **Proportional:** Scales the font size and text frame together, preserving the overall shape and arrangement of the dialogue.

The first four modes are native to Koharu. Proportional is introduced by Kipoharu.

Additional improvements:

- Alt+Scrollwheel adjusts the size of selected text.
- Proportional scaling works with rectangular and balloon-shaped text frames.
- Manually adjusting text size in Best Fit transitions to Proportional without sudden size jumps.
- Other fitting modes remain selected when manually adjusting font size.
- Fonts, sizes and fitting modes can be adjusted across multiple selected text boxes.
- Text scaling does not affect the original balloon geometry or cleaning masks.

Best Fit remains the default for automatically generated translations.

Two different types of text preserved from the original Koharu:

**Point Text:** Preserves the chosen font size and expands naturally as you type. Ideal for sound effects, floating dialogue and translator notes. No option to change text modes

**Area Text:** Offers the five fitting modes and is particularly useful for dialogue inside speech balloons. Manually drawn text boxes default to Shrink.

### Text Editing

- Escape or clicking away finishes editing without losing changes.
- Clicking away no longer creates another text box when the text canvas is open.
- Undo and redo within the text editor without needing to exit the text box.
- Multiple text boxes can be selected, restyled and duplicated.

### Story Editor

- Hidden text layers remain visible and editable within Story, with a crossed-out eye icon indicating their status.
- Visible text retains its original reading order and canvas numbering.

This makes it easier to correct OCR errors 

### Workspace and TypeBar

- The contextual TypeBar can be detached and moved freely, and when unpinned returns to it's usual placement near the selected text box.
- TypeBar positioning and visibility are remembered between sessions.
- The TypeBar can be hidden completely and restoring via shortcut or a clickable icon near the layers panel.
- A Show Original toggle in the Layers tab allows the original artwork to be viewed without disturbing existing layers.




### New/Modified Shortcuts

| Shortcut | Action |
|---|---|
| Alt+Scrollwheel | Adjust selected text size |
| Ctrl+A | Select all text boxes on the current page, excluding rasters |
| Ctrl+E | Duplicate selected text boxes |
| W/S | Navigate between pages |
| F7 | Switch between Layers and Story |
| F8 | Toggle TypeBar visibility |




### Text Colours

Choose whether processed text uses colours detected from the original manga or defaults to black text with a white outline.




### Model and Memory Management

-Improved model unloading and memory management, resulting in modest memory gains as opposed to upstream
-OCR models automatically unload after 60 seconds of inactivity.

### Font Picker

Fonts are separated into two tabs:

- **My Fonts:** Locally installed fonts, shown by default.
- **Online Fonts:** Koharu's catalogue of downloadable fonts, with previews loaded only when needed.

No more scrolling through almost 1,000 online fonts when all you want is a font already installed on your computer.




### Removed Features

- Gallery/Reader.
- Conversational AI agent and MCP server.
- Telemetry.

Upstream automatic updates have also been disabled.

### Selected Upstream Backports

- Manual OCR for selected text blocks.
- Improved handling of multiple selected text frames.
- Memory-management fixes.
- Bug fixes




## Notes

The goal isn't to substantially change Koharu or add lots of new functionality. It's mainly to remove the workflow frustrations I've encountered while translating, cleaning and typesetting manga.

Useful upstream fixes may be incorporated where appropriate, but keeping up with every new Koharu release is not a priority.