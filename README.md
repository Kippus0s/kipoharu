# Kipoharu Lite 0.92K

A workflow-focused fork of Koharu v0.92.0, with quality-of-life and productivity improvements for manga editing, cleaning and typesetting.

Kipoharu branched away from upstream following the introduction of Gallery/Reader features, and has removed the AI agent that arrived at a similar time. 
The intention is to keep the application resilient to featurecreep, and focused on translating and editing individual comics, rather than expanding into manga browsing, reading and general-purpose AI assistance.

Selected upstream bug fixes and stability fixes have also been incorporated since Koharu moved on from 0.92.

## Changes

### Layer Ordering and Cleaning

A program tailor-made for working on manga and comics should make text boxes safe from the tools included for cleaning purposes.

- New Paint and Remove layers are automatically placed below panel and text layers.
- New cleaning rasters are placed above existing cleaning rasters.
- Newly created Remove rasters are automatically selected.
- Remove can reuse the selected raster instead of creating a new layer for every application.
- Brush, Remove and Clone Stamp can work on the same raster.
- Improved brush rendering, particularly at small and fractional pixel sizes.
- Additional raster layers can still be created manually.

Cleaning and redrawing now work much more like it did in older versions of Koharu, without giving up the improvements made since then.

A Clone Stamp tool based on GIMP's controls has also been added. Press C to select it, Ctrl+click to pick a source point, and paint as you would with the brush. The tool samples the visible artwork at its original resolution, without smearing when the source and destination overlap during a stroke.




### Typesetting and Text Fitting

The TypeBar includes a drop-down for selecting how text fits within its designated area.

- **Fixed:** Preserves the specified font size.
- **Shrink:** Reduces the font size when necessary to fit the available area.
- **Grow:** Expands a rectangular text area to accommodate its contents.
- **Best Fit:** Automatically adjusts the font size to fit the available area, whilst preserving white space based on the size of the text frame. Font size can now be modified directly, switching the text box to Fixed to preserve the dimensions of the surrounding text frame. For larger changes, it is recommended to use Proportional mode to preserve neat wrapping.
- **Proportional:** Scales the font size and text frame together when manually adjusting font size. Resizing the text frame also automatically adjusts the font size using the existing Best Fit logic. Unlike Best Fit, Proportional retains the specified font size when editing the text itself, rather than continually recalculating it based on the available space. This is essentially a modified form of Best Fit intended for manual adjustment. 

The first four modes are native to Koharu. Proportional is introduced by Kipoharu. Best Fit has been tweaked as explained above, but the first three modes are the same as in 0.92 Koharu.

Additional improvements:

- Alt+Scrollwheel adjusts the size of selected text.
- The actual font size and outline thickness are displayed when automatically determined by Best Fit.
- Line spacing and a second outline, with its own colour and thickness, can be adjusted from the TypeBar. The second outline is off by default.
- Other fitting modes remain selected when manually adjusting font size. 
- Fonts, sizes and fitting modes can be adjusted across multiple selected text boxes.
- Text scaling does not affect the original balloon geometry or cleaning masks.

Best Fit remains the default for automatically generated translations.

It is important to explain the  different types of text preserved from the original Koharu and unchanged in this fork:

**Point Text:** Preserves the chosen font size and expands naturally as you type. Ideal for sound effects, floating dialogue and translator notes. No option to change text modes

**Area Text:** Offers the five fitting modes and is particularly useful for dialogue inside speech balloons. Manually drawn text boxes default to Shrink.

**Vertical text** has been adjusted for western lettering, to better suit the western style, particularly for SFX and narrow speech balloons. Letters remain upright and run down the column, groups of numbers and consecutive exclamation or question marks share a horizontal cell, and ellipsis dots run vertically. Japanese and Chinese typesetting remains unchanged. In vanilla Koharu for western lettering it essentially just rotated the text 90 degrees, a superfluous feature in this case as text can be rotated freely and easily with the Select tool already.

### Text Editing

- Escape or clicking away finishes editing without losing changes from the canvas editor
- Clicking away no longer creates another text box when the text canvas is open.
- Undo and redo within the text editor without needing to exit the text box.
- A tool with dedicated hotkey to duplicate text boxes
- Multiple text boxes can be selected, restyled and duplicated at once. 
- The font of a text box can be changed while editing it on the canvas without highlighting the text first. As one text box can not have multiple fonts in the same box simiarly to vanilla Koharu, this allows for more convenient font changes without sacrificing any control
- Bold and italic can be toggled with keyboard shortcuts where the selected font supports them.

### Story Editor

- Hidden text layers remain visible and editable within Story, with a crossed-out eye icon indicating their status.
- Visible text retains its original reading order and canvas numbering.
- Story scrolls to the selected text box when it is clicked on the canvas, not just when the f7 hotkey ported from Koharu is used. 

This makes it easier to correct OCR errors.

### Workspace and TypeBar

- The contextual TypeBar can be detached and moved freely, and when unpinned returns to it's usual placement near the selected text box.
- TypeBar positioning and visibility are remembered between sessions.
- The TypeBar can be hidden completely and restored via hotkey or a clickable icon near the layers panel.
- A Show Original toggle in the Layers tab allows the original artwork to be viewed without disturbing existing layers.
- Changes to prevent stuck tooltips which block vision of software controls, changes to tooltips also include a delay when mousing over so there is less chance of tooltips popping up unintentionally. 



### New/Modified Shortcuts

| Shortcut | Action |
|---|---|
| Alt+Scrollwheel | Adjust selected text size |
| Ctrl+A | Select all text boxes on the current page, excluding rasters | (Idea: ctrl+shift+a to select all rasters instead? ctrl+alt+shift+a to select all text and rasters?)
| Ctrl+E | Duplicate selected text boxes |
| Ctrl+B / Ctrl+I | Toggle bold / italic |
| Ctrl+L / Ctrl+Shift+L | Lock / unlock selected layers |
| Ctrl+F / Ctrl+Shift+F | Cycle forwards / backwards through favourite fonts |
| C | Select Clone Stamp |
| Ctrl+click (Clone Stamp) | Set the source point |
| W/S | Navigate between pages |
| F7 | Switch between Layers and Story |
| F8 | Toggle TypeBar visibility |




### Text Colours

Toggle the Koharu feature for processed text to use colours detected from the original manga, if disabled automatically generated text defaults black text with a white outline of 3px.

### Model and Memory Management

-Improved model unloading and memory management, resulting in modest memory gains as opposed to upstream
-OCR models automatically unload after 60 seconds of inactivity.

### Font Picker

Fonts are separated into two tabs:

- **My Fonts:** Locally installed fonts, shown by default.
- **Online Fonts:** Koharu's catalogue of downloadable fonts, with previews loaded only when needed.

No more scrolling through almost 1,000 online fonts when all you want is a font already installed on your computer.

Favourite fonts can also be cycled through using shortcuts, without having to reopen the font picker each time.




### Page Export

- Export individual pages, selected pages or the entire project in the available formats.
- Individual page exports can be given a custom filename.
- Batch exports retain the original page numbering, and automatically add version suffixes to filenames that already exist, rather than overwriting them.




### Removed Features

- Gallery/Reader.
- Conversational AI agent and MCP server.
- Telemetry.

Upstream automatic updates have also been disabled.

### Selected Upstream Backports

- Additional  Text Block tool draw text blocks missed by detection, and run OCR or clean up individual blocks from Story, unlike text created with the other text tool, it has an OCR input panel and translated text panel in the story tab
- Improvements to detection, cleaning and inpainting tone matching
- Manually made text boxes have an OCR and translated text input in the story panel
- Improved handling of locked layers and layer masks by the pipeline. 
- Improved handling of multiple selected text frames.
- Memory-management fixes.
- Various stability, rendering and UI fixes




## Notes

The goal isn't to substantially change Koharu or add lots of new functionality. It's mainly to remove the workflow frustrations I've encountered while translating, cleaning and typesetting manga.

Useful upstream fixes may be incorporated where appropriate, but keeping up with every new Koharu release is not a priority.