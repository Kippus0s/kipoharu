# Kipoharu 0.92K

A workflow-focused fork of Koharu v0.92.0, with quality-of-life improvements for manga translation, cleaning and typesetting.

Kipoharu branched away from upstream following the introduction of Gallery/Reader features. The intention is to keep the application lightweight and focused on translating and editing individual comics, rather than expanding into manga browsing and reading.

It also incorporates selected fixes from newer Koharu versions and will continue to pick up useful upstream improvements where appropriate.

## Changes

### Layer Ordering and Cleaning Workflow

A program tailor-made for working on manga and comics should make text boxes safe from the tools included for cleaning purposes.

- New Paint and Remove layers are automatically placed below panel and text layers, preventing cleaning and inpainting from covering translated text.
- New cleaning rasters are placed above existing cleaning rasters, while remaining below panels and text.
- Newly created Remove rasters are automatically selected.
- Remove can reuse the selected raster instead of unnecessarily creating a new layer for every application.
- Brush and Remove can work together on the same raster, allowing manual corrections after inpainting and repeated cleaning passes.
- Additional raster layers can still be created manually whenever needed.

These changes address some of my biggest frustrations with post-0.84 Koharu. Cleaning now works much more like it did in older versions, without giving up the improvements made since then.

### Font Picker

Fonts are separated into two tabs:

- **My Fonts:** Locally installed fonts, shown by default.
- **Online Fonts:** Koharu's catalogue of downloadable fonts, with previews loaded only when needed.

No more scrolling through almost 1,000 online fonts and waiting for previews to load when all you want is a font already installed on your computer. The online catalogue remains available if desired.

### Typesetting and Text Fitting

The contextual TypeBar includes a drop-down for selecting how text fits within its designated area.

- **Fixed:** Uses the specified font size without automatic fitting or frame resizing.
- **Shrink:** Automatically reduces the font size when necessary to fit within the text area.
- **Grow:** Allows a rectangular text area to expand along one axis to accommodate its contents.
- **Best Fit:** Automatically adjusts the font size to fit the available area.
- **Proportional:** Scales the font size and text frame dimensions together, preserving the overall shape and arrangement of the dialogue.

The first four modes are native to Koharu. Proportional is a new fitting mode introduced by Kipoharu.

**Proportional Scaling and Font Size Adjustment:**

- Alt+Scrollwheel changes text size, similarly to how it changes Paint, Eraser and Remove brush sizes.
- Proportional scaling works with both rectangular text boxes and path-shaped balloon frames.
- Text scales around its centre, preserving line wrapping, alignment and dialogue shape as closely as possible.
- Manually adjusting font size in Best Fit seamlessly transitions to Proportional, using the currently rendered font size to prevent sudden size jumps.
- Manually switching from Best Fit to Proportional also preserves the currently rendered font size.
- Other fitting modes remain selected when adjusting font size, rather than automatically switching to Fixed.
- Both TypeBar and Alt+Scrollwheel support these improvements, including multiple selected text boxes and undo/redo.
- Scaling balloon text does not affect the original balloon geometry or cleaning masks.

This makes it possible to increase or decrease the size of carefully arranged dialogue without having to manually adjust the text box dimensions or rearrange its contents.

Best Fit remains the default for automatically generated translations. Resizing the text box itself continues to adjust the font automatically, while manually adjusting the font size transitions to Proportional.

**Point Text (click on canvas):**

- Preserves your chosen font size regardless of how much text you enter.
- Expands naturally as you type.
- Doesn't unexpectedly resize your lettering when you change the wording.
- Ideal for sound effects, floating dialogue, translator notes and manually positioned text.
- Does not require a fitting mode.

**Area Text (draw a box or use a detected text region):**

- Provides control over how text fits within its designated area.
- Offers Fixed, Shrink, Grow, Best Fit and Proportional modes.
- Particularly useful for dialogue inside speech balloons.
- Grow is available for rectangular text areas but not path-shaped balloon text.
- Manually drawn text boxes default to Shrink.

### Text Editing

- Pressing Escape or clicking away finishes editing without losing changes, even with the text canvas open.
- Clicking away no longer creates another text box when a text canvas is open.
- Local undo/redo while editing text, without needing to exit the text box.

These changes allow you to finish editing a text box, close the text canvas and immediately use hotkeys to switch to another tool. Accidentally deleted text can also be restored without leaving the text editor.

### Story Editor

- Hidden text layers remain visible and editable within the Story tab, with a crossed-out eye icon indicating their hidden status.
- Visible text retains its original reading order and canvas numbering.

This allows OCR errors to be corrected by hiding the translated text and referring directly to the original Japanese on the canvas, without losing access to the source and translation fields in Story.

Previously, hiding a text layer would also remove its entry from Story, making OCR correction and proofreading unnecessarily awkward.

### Workspace and TypeBar

- The contextual TypeBar can be detached, moved freely around the canvas and pinned in place.
- TypeBar positioning and visibility are remembered between sessions.
- The TypeBar can be hidden or restored using a dedicated visibility control or the F8 shortcut.
- Quickly switch between Layers and Story using F7.

These changes provide greater control over the available canvas space, particularly when working on dialogue near the edges of a page or when the contextual toolbar obstructs the artwork.

### Shortcuts

- **Alt+Scrollwheel:** Adjust the font size of selected text.
- **Ctrl+A:** Select all text boxes on the current page, including those inside panels and balloons.
- **Ctrl+E:** Duplicate selected text boxes.
- **W/S:** Navigate between pages, particularly useful for proofreading.
- **K:** Run OCR on a selected text block.
- **F7:** Toggle between Layers and Story.
- **F8:** Toggle TypeBar visibility.

**Text Duplication:**

Text duplication makes use of Koharu's existing native layer functionality rather than introducing a separate duplication system.

This is particularly useful when typesetting repeated dialogue, sound effects or other text that shares the same formatting, without needing to recreate text boxes and adjust their settings each time.

### Text Colours

Added a setting to choose whether processed text uses colours detected from the original manga or defaults to black text with a white outline.

### Selected Upstream Backports

Useful fixes and improvements from newer Koharu versions have been selectively incorporated without introducing Gallery/Reader functionality.

These include:

- Memory-management fixes.
- Cleaning and inpainting improvements.
- Improved layer deletion and movement.
- Offline functionality improvements.
- Manual OCR for selected text blocks.
- Improved handling of mixed text fitting modes and multiple selected text frames.
- Keyboard navigation between Layers and Story.

## Notes

The goal isn't to substantially change Koharu or add lots of new functionality. It's mainly to remove the workflow frustrations I've encountered while translating, cleaning and typesetting manga.

I'll continue to incorporate useful upstream bug fixes and make practical workflow improvements where needed, while keeping the application focused on manga translation and editing and retaining the foundations of Koharu 0.92.0.