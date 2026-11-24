# Kipoharu 0.92K

A workflow-focused fork of Koharu v0.92.0, with some quality-of-life improvements for manga translation, cleaning, and typesetting.

Kipoharu branched away from upstream following the introduction of Gallery/Reader features. The intention is to keep the application lightweight and focused on translating and editing individual comics, rather than expanding into manga browsing and reading.

It also incorporates selected memory fixes from newer Koharu versions and will continue to pick up useful upstream fixes where appropriate.

## Changes

### Layer Ordering

A program which is tailor-made for working on manga and comics should make text boxes safe from the tools included for cleaning purposes.

- New Paint and Remove layers are automatically placed below the panel and text layers, so cleaning and inpainting no longer cover translated text.

This would otherwise result in needing to manually reorder the text layers frequently, and so fixes one of my biggest frustrations with post-0.84 Koharu. Cleaning now works much more like it did in older versions, without giving up the improvements made since then.

### Improved Brush and Remove Workflow

Similar to the layer ordering changes, but with respect to how the Remove tool utilises the new layering system, and how the Brush and Remove tools interact. Again, without these changes, performing bespoke cleaning work was frustrating and significantly slower at best.

- New cleaning rasters are placed above existing cleaning rasters, while remaining below panels and text.
- Newly created Remove rasters are automatically selected by default.
- Remove can reuse the selected raster instead of unnecessarily creating a new layer for every application of the Remove tool.
- Brush and Remove can work together on the same raster, allowing manual corrections after inpainting and repeated cleaning passes.
- Additional raster layers can still be created manually whenever needed.

### Font Picker

Fonts are separated into two tabs:

- **My Fonts:** Locally installed fonts, shown by default.
- **Online Fonts:** Koharu's catalogue of downloadable fonts, with previews loaded only when needed.

No more scrolling through almost 1,000 online fonts and waiting for previews to load when all you want is a font already installed on your computer. The online catalogue is still available if desired.

### Text Size Adjustment

- **Alt+Scrollwheel:** Changes text size, similarly to how Alt+Scrollwheel changes Paint, Eraser and Remove brush sizes.

### Text Editing

- Pressing Escape or clicking away finishes editing without losing changes, even with the text canvas open.
- Clicking away no longer creates another text box when a text canvas is open.
- Local undo/redo while editing text, without needing to exit the text box. Accidentally deleted text inside a canvas can be restored in this way.

The first two changes mean you can finish editing a text box and then close the text canvas by pressing Escape or clicking away, without losing changes, and then be free to use hotkeys to switch to a different tool.

### Text Fitting Modes

The contextual text bar now includes a drop-down for selecting how text fits within its designated area:
The following modes were built-in to Koharu, and were not implemented here, the drop-down box simply allows the user to select between them at will. Info on each mode below.

- **Fixed:** Uses the specified font size without automatic fitting.
- **Shrink:** Automatically reduces the font size when necessary to fit within the text area.
- **Grow:** Allows a rectangular text area to expand to accommodate its contents.
- **Best Fit:** Automatically adjusts the font size to fit the available area.

Point text retains its original behavior, providing a predictable way to create fixed-size lettering without automatic fitting. If a text box is created by clicking on the canvas with the Text tool, it will remain in Fixed mode and will not show the dropdown.

**Point text (click on canvas):**

- Preserves your chosen font size regardless of how much text you enter.
- Expands naturally as you type.
- Doesn't unexpectedly resize your lettering when you change the wording.
- Ideal for sound effects, floating dialogue, translator notes and manually positioned text.

**Area text (draw a box or use a detected text region):**

- Provides control over how text fits within its designated area.
- Offers Fixed, Shrink, Grow and Best Fit modes.
- Particularly useful for dialogue inside speech balloons.
- Grow is available for rectangular text areas but not path-shaped balloon text.

### Shortcuts


- **Ctrl+A:** Select all text boxes on the current page, including those inside panels and balloons.
- **W/S:** Navigate between pages, particularly useful for proofreading.
  
**Text Duplication**:
- **Ctrl+E:** Duplicate selected text boxes.

Although this is a new feature, it makes use of Koharu's existing native layer functionality rather than introducing a separate duplication system.
This is particularly useful when typesetting repeated dialogue, sound effects or other text that shares the same formatting, without needing to recreate text boxes and adjust their settings each time.




### Text Colours

Added a setting to choose whether processed text uses colours detected from the original manga or defaults to black text with a white outline.

### Memory Fixes

Cherry-picked selected memory-management fixes from newer upstream Koharu versions.

## Notes

The goal isn't to substantially change Koharu or add lots of new functionality. It's mainly to remove the workflow frustrations I've encountered while translating and typesetting manga.

I'll continue to incorporate useful upstream bug fixes and make small improvements where needed, but otherwise intend to keep things fairly close to 0.92.0.
